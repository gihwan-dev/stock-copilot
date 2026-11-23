type ElementView = Readonly<Element>;
type StyleView = Readonly<CSSStyleDeclaration>;
type DocumentView = Readonly<Document>;
export const isViewport = (element: ElementView): boolean => {
  if (!element.hasAttribute("data-scroll-viewport")) return false;
  const style = getComputedStyle(element);
  return [style.overflowX, style.overflowY].some((value) =>
    ["auto", "scroll", "overlay"].includes(value),
  );
};
export const isTruncated = (element: ElementView): boolean => {
  if (!element.hasAttribute("data-truncate")) return false;
  const style = getComputedStyle(element);
  const clamp = style.getPropertyValue("-webkit-line-clamp");
  return (
    (style.overflowX !== "visible" || style.overflowY !== "visible") &&
    (style.textOverflow === "ellipsis" ||
      (clamp !== "" && clamp !== "none" && clamp !== "0"))
  );
};
const activeModal = (doc: DocumentView): Element | null | undefined => {
  const modals = doc.querySelectorAll("dialog:modal");
  if (modals.length === 0) return null;
  if (modals.length === 1) return modals.item(0);
  return doc.activeElement?.closest("dialog:modal") ?? undefined;
};
export const modalOrderUnknown = (doc: DocumentView): boolean =>
  activeModal(doc) === undefined;
const inertWithin = (
  el: ElementView | null,
  stop: ElementView | null,
): boolean =>
  el !== null &&
  (el.hasAttribute("inert") ||
    getComputedStyle(el).getPropertyValue("interactivity") === "inert" ||
    (el !== stop && inertWithin(el.parentElement, stop)));
export const isEditable = (element: ElementView): boolean => {
  const modal = activeModal(element.ownerDocument);
  return (
    modal !== undefined &&
    (modal === null || modal.contains(element)) &&
    !inertWithin(element, modal)
  );
};
export const scrollableTextarea = (element: ElementView): boolean =>
  element.matches("textarea:not([readonly]):not(:disabled)") &&
  ["auto", "scroll"].includes(getComputedStyle(element).overflowY);
export const noGeneratedContent = (element: ElementView): boolean =>
  ["none", "normal"].includes(getComputedStyle(element, "::before").content) &&
  ["none", "normal"].includes(getComputedStyle(element, "::after").content);
type Escape = "none" | "absolute" | "fixed";
const containingEffects =
  "transform translate rotate scale perspective filter backdrop-filter".split(
    " ",
  );
const establishesContainingBlock = (style: StyleView): boolean =>
  containingEffects.some(
    (property) =>
      !["", "none"].includes(style.getPropertyValue(property)) ||
      style.willChange.split(/\s*,\s*/u).includes(property),
  ) ||
  ["layout", "paint", "strict", "content"].some((value) =>
    style.contain.split(/\s+/u).includes(value),
  );
const nextEscape = (position: string, escaping: Escape): Escape => {
  if (escaping !== "none") return escaping;
  return position === "fixed" || position === "absolute" ? position : "none";
};
const settledEscape = (pending: Escape, style: StyleView): Escape => {
  if (pending === "none") return "none";
  if (establishesContainingBlock(style)) return "none";
  if (pending === "absolute" && style.position !== "static") return "none";
  return pending;
};
const clipsOverflow = (style: StyleView): boolean =>
  style.overflowX !== "visible" || style.overflowY !== "visible";
const clipsPaint = (style: StyleView): boolean =>
  ["paint", "strict", "content"].some((value) =>
    style.contain.split(/\s+/u).includes(value),
  );
const clips = (style: StyleView): boolean =>
  clipsOverflow(style) || clipsPaint(style);
const bodyClipsViewport = (parent: ElementView, style: StyleView): boolean => {
  const root = parent.ownerDocument.documentElement;
  if (
    parent !== parent.ownerDocument.body ||
    !clipsOverflow(style) ||
    clipsPaint(style)
  )
    return false;
  const rootStyle = getComputedStyle(root);
  return rootStyle.overflowX === "visible" && rootStyle.overflowY === "visible";
};
const nearestClipFrom = (el: ElementView, escape: Escape): Element | null => {
  const parent = el.parentElement;
  if (parent === null) return null;
  const pending = nextEscape(getComputedStyle(el).position, escape);
  const style = getComputedStyle(parent);
  const settled = settledEscape(pending, style);
  const root = el.ownerDocument.documentElement;
  if (parent === root) return clips(style) ? parent : null;
  if (bodyClipsViewport(parent, style)) return root;
  return settled === "none" && clips(style)
    ? parent
    : nearestClipFrom(parent, settled);
};
const isScrollable = (value: string): boolean =>
  ["auto", "scroll", "overlay"].includes(value);
type Interval = { readonly start: number; readonly end: number };
type Axis = {
  readonly offset: number;
  readonly extent: number;
  readonly canScroll: boolean;
  readonly reverse: boolean;
};
const outsideReach = (inner: Interval, clip: Interval, axis: Axis): boolean => {
  if (!axis.canScroll) return inner.start < clip.start || inner.end > clip.end;
  const minimum = axis.reverse ? -axis.extent : 0;
  const maximum = axis.reverse ? 0 : axis.extent;
  return (
    inner.start < clip.start - (axis.offset - minimum) ||
    inner.end > clip.end + (maximum - axis.offset)
  );
};
type ClipBounds = Pick<DOMRectReadOnly, "left" | "right" | "top" | "bottom">;
const reversedFlex = (style: StyleView, direction: string): boolean =>
  ["flex", "inline-flex"].includes(style.display) &&
  style.flexDirection === direction;
const viewportClipped = (
  parent: ElementView,
  inner: Readonly<DOMRect>,
  bounds: Readonly<ClipBounds>,
): boolean => {
  const style = getComputedStyle(parent);
  return (
    outsideReach(
      { start: inner.left, end: inner.right },
      { start: bounds.left, end: bounds.right },
      {
        offset: parent.scrollLeft,
        extent: parent.scrollWidth - parent.clientWidth,
        canScroll: isScrollable(style.overflowX),
        reverse:
          (style.direction === "rtl") !== reversedFlex(style, "row-reverse"),
      },
    ) ||
    outsideReach(
      { start: inner.top, end: inner.bottom },
      { start: bounds.top, end: bounds.bottom },
      {
        offset: parent.scrollTop,
        extent: parent.scrollHeight - parent.clientHeight,
        canScroll: isScrollable(style.overflowY),
        reverse: reversedFlex(style, "column-reverse"),
      },
    )
  );
};
export const clipped = (element: ElementView): boolean => {
  const parent = nearestClipFrom(element, "none");
  if (parent === null) return false;
  const bounds = parent.getBoundingClientRect();
  const root = element.ownerDocument.documentElement;
  const left = parent === root ? 0 : bounds.left + parent.clientLeft;
  const top = parent === root ? 0 : bounds.top + parent.clientTop;
  const right = left + parent.clientWidth;
  const bottom = top + parent.clientHeight;
  const inner = element.getBoundingClientRect();
  if (isViewport(parent))
    return viewportClipped(parent, inner, { left, right, top, bottom });
  return (
    inner.left < left ||
    inner.top < top ||
    inner.right > right ||
    inner.bottom > bottom
  );
};
export const childInside = (host: ElementView, child: ElementView): boolean => {
  const inner = child.getBoundingClientRect();
  const outer = host.getBoundingClientRect();
  const left = outer.left + host.clientLeft;
  const top = outer.top + host.clientTop;
  return (
    inner.left >= left &&
    inner.right <= left + host.clientWidth &&
    inner.top >= top &&
    inner.bottom <= top + host.clientHeight
  );
};

const irregularClip = (style: StyleView): boolean =>
  style.clipPath !== "none" ||
  style.getPropertyValue("clip") !== "auto" ||
  "mask-image -webkit-mask-image -webkit-mask-box-image-source mask-border-source"
    .split(" ")
    .some(
      (property) => !["", "none"].includes(style.getPropertyValue(property)),
    ) ||
  clipsPaint(style);
const textOutsideClip = (
  parent: ElementView,
  text: Readonly<DOMRect>,
  style: StyleView,
): boolean => {
  const bounds = parent.getBoundingClientRect();
  const left = bounds.left + parent.clientLeft;
  const top = bounds.top + parent.clientTop;
  return (
    (style.overflowX !== "visible" &&
      (text.left < left || text.right > left + parent.clientWidth)) ||
    (style.overflowY !== "visible" &&
      (text.top < top || text.bottom > top + parent.clientHeight))
  );
};
export const textInsideClippingAncestors = (
  parent: ElementView | null,
  text: Readonly<DOMRect>,
): boolean => {
  if (parent === null) return true;
  const style = getComputedStyle(parent);
  if (irregularClip(style)) return false;
  if (!bodyClipsViewport(parent, style) && textOutsideClip(parent, text, style))
    return false;
  return textInsideClippingAncestors(parent.parentElement, text);
};
const isScreenReaderClip = (element: ElementView): boolean => {
  const style = getComputedStyle(element);
  return (
    style.position === "absolute" &&
    style.overflowX === "hidden" &&
    style.overflowY === "hidden" &&
    (style.getPropertyValue("clip") === "rect(0px, 0px, 0px, 0px)" ||
      style.clipPath === "inset(50%)")
  );
};
export const isScreenReaderTable = (element: ElementView): boolean => {
  const table = element.matches("div.sr-only")
    ? element.querySelector(":scope > table.sr-only")
    : element.closest("table.sr-only");
  if (table === null) return false;
  const wrapper = table.parentElement;
  if (wrapper === null) return false;
  return (
    wrapper.matches("div.sr-only") &&
    wrapper.getBoundingClientRect().width <= 1 &&
    wrapper.getBoundingClientRect().height <= 1 &&
    isScreenReaderClip(wrapper) &&
    isScreenReaderClip(table)
  );
};
