import {
  clipped,
  isScreenReaderTable,
  isTruncated,
  isViewport,
  textInsideClippingAncestors,
  editableScrollableTextarea,
} from "./overflow-geometry";

type OverflowKind =
  | "document-scroll"
  | "unauthorized-overflow"
  | "clipped"
  | "missing-tooltip"
  | "nested-scroll"
  | "outside-window"
  | "overlapping-controls"
  | "layout-unavailable";

export type OverflowFinding = {
  readonly kind: OverflowKind;
  readonly element: string;
};
const nameOf = (element: Readonly<Element>): string =>
  `${element.tagName.toLowerCase()}${element.id === "" ? "" : `#${element.id}`}`;

const finding = (
  kind: OverflowKind,
  element: Readonly<Element>,
): OverflowFinding => ({
  kind,
  element: nameOf(element),
});
const hasOverflow = (element: Readonly<Element>): boolean =>
  element.scrollWidth > element.clientWidth ||
  element.scrollHeight > element.clientHeight;

const editableTextInput = (element: Readonly<Element>): boolean =>
  element.matches("input:not([readonly]):not(:disabled)") &&
  ["text", "search", "tel", "url", "email", "password"].includes(
    (element.getAttribute("type") ?? "text").toLowerCase(),
  );
const hasOnePixelTextOverflow = (element: Readonly<Element>): boolean =>
  element.scrollHeight === element.clientHeight + 1 &&
  element.scrollWidth <= element.clientWidth &&
  element.childNodes.length === 1 &&
  element.childNodes[0]?.nodeType === 3 &&
  element.textContent.trim() !== "";

const hasMetricStyle = (
  element: Readonly<Element>,
  style: Readonly<CSSStyleDeclaration>,
): boolean =>
  style.fontFamily.split(",")[0]?.replace(/["']/gu, "").trim() ===
    "IBM Plex Sans KR" &&
  style.overflowX === "visible" &&
  style.overflowY === "visible" &&
  ["none", "normal"].includes(getComputedStyle(element, "::before").content) &&
  ["none", "normal"].includes(getComputedStyle(element, "::after").content);
const textMatchesMetric = (
  text: Readonly<DOMRect>,
  bounds: Readonly<DOMRect>,
): boolean =>
  text.left >= bounds.left &&
  text.right <= bounds.right &&
  text.top >= bounds.top - 1 &&
  text.bottom <= bounds.bottom + 1;

const isVisibleFontMetric = (element: Readonly<Element>): boolean => {
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
        textInsideClippingAncestors(element.parentElement, text),
    )
  );
};
const hasUnpermittedContentOverflow = (element: Readonly<Element>): boolean =>
  (element.scrollWidth > element.clientWidth && !editableTextInput(element)) ||
  (element.scrollHeight > element.clientHeight &&
    !editableScrollableTextarea(element) &&
    !isVisibleFontMetric(element));

const isVisible = (element: Readonly<Element>): boolean =>
  element.getClientRects().length !== 0;
const trimmedText = (text: string | null): string =>
  (text ?? "").replace(/\s+/gu, " ").trim();

const hasFullTooltip = (
  element: Readonly<Element>,
  doc: Readonly<Document>,
): boolean => {
  const text = trimmedText(element.textContent);
  const title = trimmedText(element.getAttribute("title"));
  const ids = (element.getAttribute("aria-describedby") ?? "")
    .trim()
    .split(/\s+/u);
  return (
    text !== "" &&
    (title === text ||
      ids.some((id) => {
        const tooltip = doc.getElementById(id);
        return (
          tooltip?.getAttribute("role") === "tooltip" &&
          trimmedText(tooltip.textContent) === text
        );
      }))
  );
};
const verticalViewport = (element: Readonly<Element>): boolean => {
  const overflow = getComputedStyle(element).overflowY;
  return (
    (isViewport(element) || element.matches("textarea")) &&
    ["auto", "scroll", "overlay"].includes(overflow)
  );
};

const hasVerticalAncestor = (element: Readonly<Element>): boolean => {
  const parent = element.parentElement?.closest("[data-scroll-viewport]");
  return (
    parent !== null &&
    parent !== undefined &&
    (verticalViewport(parent) || hasVerticalAncestor(parent))
  );
};
const nestedViewport = (element: Readonly<Element>): boolean =>
  verticalViewport(element) &&
  !element.matches("textarea.chat-input") &&
  hasVerticalAncestor(element);

const outsideWindow = (
  bounds: Readonly<DOMRect>,
  doc: Readonly<Document>,
): boolean =>
  bounds.left < 0 ||
  bounds.top < 0 ||
  bounds.right > doc.documentElement.clientWidth ||
  bounds.bottom > doc.documentElement.clientHeight;
const overlaps = (
  first: Readonly<Element>,
  second: Readonly<Element>,
): boolean => {
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
const overlapsSibling = (element: Readonly<Element>): boolean =>
  element.matches(controls) &&
  [...(element.parentElement?.children ?? [])].some(
    (other) =>
      other !== element &&
      other.matches(controls) &&
      isVisible(other) &&
      overlaps(element, other),
  );
const unauthorized = (element: Readonly<Element>): boolean =>
  hasUnpermittedContentOverflow(element) &&
  (!isViewport(element) || element.matches("input,textarea")) &&
  !isTruncated(element);
const missingTooltip = (
  element: Readonly<Element>,
  doc: Readonly<Document>,
): boolean =>
  element.hasAttribute("data-truncate") && !hasFullTooltip(element, doc);
const outsideOverlay = (
  element: Readonly<Element>,
  doc: Readonly<Document>,
): boolean =>
  element.matches(overlays) &&
  outsideWindow(element.getBoundingClientRect(), doc);
const reportIf = (
  condition: boolean,
  kind: OverflowKind,
  element: Readonly<Element>,
): readonly OverflowFinding[] => (condition ? [finding(kind, element)] : []);
const findingsFor = (
  element: Readonly<Element>,
  doc: Readonly<Document>,
): readonly OverflowFinding[] => [
  ...reportIf(
    !isScreenReaderTable(element) && unauthorized(element),
    "unauthorized-overflow",
    element,
  ),
  ...reportIf(
    !isScreenReaderTable(element) && clipped(element),
    "clipped",
    element,
  ),
  ...reportIf(missingTooltip(element, doc), "missing-tooltip", element),
  ...reportIf(nestedViewport(element), "nested-scroll", element),
  ...reportIf(outsideOverlay(element, doc), "outside-window", element),
  ...reportIf(overlapsSibling(element), "overlapping-controls", element),
];
export const inspectOverflow = (
  doc: Readonly<Document>,
): readonly OverflowFinding[] => {
  if (
    doc.documentElement.clientWidth === 0 ||
    doc.documentElement.clientHeight === 0
  )
    return [finding("layout-unavailable", doc.documentElement)];
  if (getComputedStyle(doc.documentElement).display === "none")
    return [finding("layout-unavailable", doc.documentElement)];
  const body = doc.querySelector("body");
  if (body === null)
    return [finding("layout-unavailable", doc.documentElement)];
  if (getComputedStyle(body).display === "none")
    return [finding("layout-unavailable", body)];
  const elements = [
    doc.documentElement,
    ...doc.querySelectorAll("body, body *"),
  ];
  const documentFindings = [doc.documentElement, body]
    .filter(hasOverflow)
    .map((element) => finding("document-scroll", element));
  return [
    ...documentFindings,
    ...elements
      .filter(isVisible)
      .flatMap((element) => findingsFor(element, doc)),
  ];
};
