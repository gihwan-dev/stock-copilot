type OverflowKind =
  | "document-scroll"
  | "unauthorized-overflow"
  | "clipped"
  | "missing-tooltip"
  | "nested-scroll"
  | "outside-window"
  | "overlapping-controls";

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

const isVisible = (element: Readonly<Element>): boolean =>
  element.getClientRects().length !== 0;

const isViewport = (element: Readonly<Element>): boolean =>
  element.hasAttribute("data-scroll-viewport");

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

const nearestClip = (element: Readonly<Element>): Element | null => {
  const parent = element.parentElement;
  if (parent === null) return null;
  const style = getComputedStyle(parent);
  return style.overflowX !== "visible" || style.overflowY !== "visible"
    ? parent
    : nearestClip(parent);
};

const clipped = (element: Readonly<Element>): boolean => {
  const parent = nearestClip(element);
  if (parent === null || isViewport(parent)) return false;
  const outer = parent.getBoundingClientRect();
  const inner = element.getBoundingClientRect();
  return (
    inner.left < outer.left ||
    inner.top < outer.top ||
    inner.right > outer.right ||
    inner.bottom > outer.bottom
  );
};

const verticalViewport = (element: Readonly<Element>): boolean => {
  const overflow = getComputedStyle(element).overflowY;
  return (
    isViewport(element) && ["auto", "scroll", "overlay"].includes(overflow)
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
  element: Readonly<Element>,
  doc: Readonly<Document>,
): boolean => {
  const bounds = element.getBoundingClientRect();
  return (
    bounds.left < 0 ||
    bounds.top < 0 ||
    bounds.right > doc.documentElement.clientWidth ||
    bounds.bottom > doc.documentElement.clientHeight
  );
};

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

const controls = "button,input,select,textarea,a[href],[role='button']";
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
  hasOverflow(element) &&
  !isViewport(element) &&
  !element.hasAttribute("data-truncate");

const missingTooltip = (
  element: Readonly<Element>,
  doc: Readonly<Document>,
): boolean =>
  element.hasAttribute("data-truncate") && !hasFullTooltip(element, doc);

const outsideOverlay = (
  element: Readonly<Element>,
  doc: Readonly<Document>,
): boolean => element.matches(overlays) && outsideWindow(element, doc);

const reportIf = (
  condition: boolean,
  kind: OverflowKind,
  element: Readonly<Element>,
): readonly OverflowFinding[] => (condition ? [finding(kind, element)] : []);

const findingsFor = (
  element: Readonly<Element>,
  doc: Readonly<Document>,
): readonly OverflowFinding[] => [
  ...reportIf(unauthorized(element), "unauthorized-overflow", element),
  ...reportIf(clipped(element), "clipped", element),
  ...reportIf(missingTooltip(element, doc), "missing-tooltip", element),
  ...reportIf(nestedViewport(element), "nested-scroll", element),
  ...reportIf(outsideOverlay(element, doc), "outside-window", element),
  ...reportIf(overlapsSibling(element), "overlapping-controls", element),
];

export const inspectOverflow = (
  doc: Readonly<Document>,
): readonly OverflowFinding[] => {
  const elements = [
    doc.documentElement,
    ...doc.querySelectorAll("body, body *"),
  ];
  const documentFindings = [doc.documentElement, doc.body]
    .filter(hasOverflow)
    .map((element) => finding("document-scroll", element));
  return [
    ...documentFindings,
    ...elements
      .filter(isVisible)
      .flatMap((element) => findingsFor(element, doc)),
  ];
};
