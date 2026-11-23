import {
  childInside,
  clipped,
  isScreenReaderTable,
  isEditable,
  isTruncated,
  isViewport,
  modalOrderUnknown,
  noGeneratedContent,
  textInsideClippingAncestors,
  scrollableTextarea,
} from "./overflow-geometry";

type ElementView = Readonly<Element>;
type StyleView = Readonly<CSSStyleDeclaration>;
type DocumentView = Readonly<Document>;
type RectView = Readonly<DOMRect>;
type Kind =
  | "document-scroll"
  | "unauthorized-overflow"
  | "clipped"
  | "missing-tooltip"
  | "nested-scroll"
  | "outside-window"
  | "overlapping-controls"
  | "layout-unavailable";
export type OverflowFinding = {
  readonly kind: Kind;
  readonly element: string;
};
type Finding = OverflowFinding;
type Namer = (element: ElementView) => string;
const nameOf = (element: ElementView): string =>
  `${element.tagName.toLowerCase()}${element.id === "" ? "" : `#${element.id}`}`;
const finding = (kind: Kind, el: ElementView, name: Namer): Finding => ({
  kind,
  element: name(el),
});
const hasOverflow = (element: ElementView): boolean =>
  element.scrollWidth > element.clientWidth ||
  element.scrollHeight > element.clientHeight;
const textInput = (element: ElementView): boolean =>
  element.matches("input:not([readonly]):not(:disabled)") &&
  ["text", "search", "tel", "url", "email", "password", "number"].includes(
    element.closest("input")?.type ?? "",
  );
const editableTextInput = (element: ElementView): boolean =>
  textInput(element) && isEditable(element);
const onePixelOverflow = (element: ElementView): boolean =>
  element.scrollHeight === element.clientHeight + 1 &&
  element.scrollWidth <= element.clientWidth;
const hasOnePixelTextOverflow = (element: ElementView): boolean =>
  onePixelOverflow(element) &&
  element.childNodes.length === 1 &&
  element.childNodes[0]?.nodeType === 3 &&
  element.textContent.trim() !== "";
const visibleOverflow = (style: StyleView): boolean =>
  style.overflowX === "visible" && style.overflowY === "visible";
const hasMetricStyle = (element: ElementView, style: StyleView): boolean =>
  style.fontFamily.split(",")[0]?.replace(/["']/gu, "").trim() ===
    "IBM Plex Sans KR" &&
  visibleOverflow(style) &&
  noGeneratedContent(element);
const textMatchesMetric = (text: RectView, bounds: RectView): boolean =>
  text.left >= bounds.left &&
  text.right <= bounds.right &&
  text.top >= bounds.top - 1 &&
  text.bottom <= bounds.bottom + 1;

const isVisibleFontMetric = (element: ElementView): boolean => {
  if (!hasOnePixelTextOverflow(element)) return false;
  const style = getComputedStyle(element);
  if (!hasMetricStyle(element, style)) return false;
  const range = element.ownerDocument.createRange();
  range.selectNodeContents(element);
  const rects = [...range.getClientRects()];
  const first = rects[0];
  const last = rects.at(-1);
  if (first === undefined || last === undefined) return false;
  const bounds = element.getBoundingClientRect();
  return (
    first.top < bounds.top &&
    last.bottom > bounds.bottom &&
    rects.every(
      (text) =>
        textMatchesMetric(text, bounds) &&
        !outsideWindow(text, element.ownerDocument) &&
        textInsideClippingAncestors(element, text),
    )
  );
};
const containsVisibleMetric = (element: ElementView): boolean => {
  const child = element.firstElementChild;
  if (
    child === null ||
    element.childNodes.length !== 1 ||
    !onePixelOverflow(element)
  )
    return false;
  if (
    !visibleOverflow(getComputedStyle(element)) ||
    !noGeneratedContent(element)
  )
    return false;
  return (
    childInside(element, child) &&
    (isVisibleFontMetric(child) || containsVisibleMetric(child))
  );
};
const hasUnpermittedContentOverflow = (element: ElementView): boolean =>
  (element.scrollWidth > element.clientWidth && !editableTextInput(element)) ||
  (element.scrollHeight > element.clientHeight &&
    !(scrollableTextarea(element) && isEditable(element)) &&
    !isVisibleFontMetric(element) &&
    !containsVisibleMetric(element));

const isVisible = (element: ElementView): boolean =>
  element.getClientRects().length !== 0;
const modalUnavailable = (doc: DocumentView): boolean =>
  modalOrderUnknown(doc) &&
  [...doc.querySelectorAll("input,textarea")].some(
    (element) =>
      isVisible(element) &&
      ((textInput(element) && element.scrollWidth > element.clientWidth) ||
        (scrollableTextarea(element) &&
          element.scrollHeight > element.clientHeight)),
  );
const trimmedText = (text: string | null): string =>
  (text ?? "").replace(/\s+/gu, " ").trim();
const hasFullTooltip = (element: ElementView): boolean => {
  const text = trimmedText(element.textContent);
  if (text === "") return false;
  if (trimmedText(element.getAttribute("title")) === text) return true;
  const ids = (element.getAttribute("aria-describedby") ?? "")
    .trim()
    .split(/\s+/u);
  return ids.some((id) => {
    const tooltip = element.ownerDocument.getElementById(id);
    return (
      tooltip?.getAttribute("role") === "tooltip" &&
      trimmedText(tooltip.textContent) === text
    );
  });
};
const verticalViewport = (element: ElementView): boolean => {
  const overflow = getComputedStyle(element).overflowY;
  return (
    (isViewport(element) || element.matches("textarea")) &&
    ["auto", "scroll", "overlay"].includes(overflow)
  );
};

const hasVerticalAncestor = (element: ElementView): boolean => {
  const parent =
    element.parentElement?.closest("[data-scroll-viewport]") ?? null;
  return (
    parent !== null && (verticalViewport(parent) || hasVerticalAncestor(parent))
  );
};
const nestedViewport = (element: ElementView): boolean =>
  verticalViewport(element) &&
  !element.matches("textarea.chat-input") &&
  hasVerticalAncestor(element);

const outsideWindow = (bounds: RectView, doc: DocumentView): boolean =>
  bounds.left < 0 ||
  bounds.top < 0 ||
  bounds.right > doc.documentElement.clientWidth ||
  bounds.bottom > doc.documentElement.clientHeight;
const overlaps = (first: ElementView, second: ElementView): boolean => {
  const a = first.getBoundingClientRect();
  const b = second.getBoundingClientRect();
  return (
    a.width > 0 &&
    a.height > 0 &&
    b.width > 0 &&
    b.height > 0 &&
    a.left < b.right &&
    b.left < a.right &&
    a.top < b.bottom &&
    b.top < a.bottom
  );
};

const controls =
  "button,input,select,textarea,a[href],[role='button'],[role='switch']";
const overlays =
  "[role='dialog'],[role='alertdialog'],[role='menu'],[role='listbox'],[role='tooltip'],[popover],[data-overflow-overlay]";
const overlapsSibling = (element: ElementView): boolean =>
  element.matches(controls) &&
  [...(element.parentElement?.children ?? [])].some(
    (other) =>
      other !== element &&
      other.matches(controls) &&
      isVisible(other) &&
      overlaps(element, other),
  );
const unauthorized = (element: ElementView): boolean =>
  hasUnpermittedContentOverflow(element) &&
  (!isViewport(element) || element.matches("input,textarea")) &&
  (!isTruncated(element) || element.matches("input,textarea"));
const missingTooltip = (element: ElementView): boolean =>
  element.hasAttribute("data-truncate") && !hasFullTooltip(element);
const outsideOverlay = (element: ElementView): boolean =>
  element.matches(overlays) &&
  outsideWindow(element.getBoundingClientRect(), element.ownerDocument);
const findingsFor = (
  element: ElementView,
  describe: Namer,
): readonly OverflowFinding[] => {
  const visible = !isScreenReaderTable(element);
  const checks = [
    [visible && unauthorized(element), "unauthorized-overflow"],
    [visible && clipped(element), "clipped"],
    [missingTooltip(element), "missing-tooltip"],
    [nestedViewport(element), "nested-scroll"],
    [outsideOverlay(element), "outside-window"],
    [overlapsSibling(element), "overlapping-controls"],
  ] as const;
  return checks.flatMap(([found, kind]) =>
    found ? [finding(kind, element, describe)] : [],
  );
};
export const inspectOverflow = (
  doc: DocumentView,
  describe: Namer = nameOf,
): readonly OverflowFinding[] => {
  const root = doc.documentElement;
  if (
    root.clientWidth === 0 ||
    root.clientHeight === 0 ||
    getComputedStyle(root).display === "none"
  )
    return [finding("layout-unavailable", root, describe)];
  const body = doc.querySelector("body");
  if (body === null) return [finding("layout-unavailable", root, describe)];
  if (getComputedStyle(body).display === "none")
    return [finding("layout-unavailable", body, describe)];
  return [
    ...(modalUnavailable(doc)
      ? [finding("layout-unavailable", root, describe)]
      : []),
    ...[root, body]
      .filter(hasOverflow)
      .map((element) => finding("document-scroll", element, describe)),
    ...[root, ...doc.querySelectorAll("body, body *")]
      .filter(isVisible)
      .flatMap((element) => findingsFor(element, describe)),
  ];
};
