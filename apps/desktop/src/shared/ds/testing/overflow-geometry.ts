export const isViewport = (element: Readonly<Element>): boolean => {
  if (!element.hasAttribute("data-scroll-viewport")) return false;
  const style = getComputedStyle(element);
  return [style.overflowX, style.overflowY].some((value) =>
    ["auto", "scroll", "overlay"].includes(value),
  );
};
export const isTruncated = (element: Readonly<Element>): boolean => {
  if (!element.hasAttribute("data-truncate")) return false;
  const style = getComputedStyle(element);
  const clamp = style.getPropertyValue("-webkit-line-clamp");
  return (
    (style.overflowX !== "visible" || style.overflowY !== "visible") &&
    (style.textOverflow === "ellipsis" ||
      (clamp !== "" && clamp !== "none" && clamp !== "0"))
  );
};
export const editableScrollableTextarea = (
  element: Readonly<Element>,
): boolean =>
  element.matches("textarea:not([readonly]):not(:disabled)") &&
  element.closest("[inert]") === null &&
  ["auto", "scroll"].includes(getComputedStyle(element).overflowY);
export const noGeneratedContent = (element: Readonly<Element>): boolean =>
  ["none", "normal"].includes(getComputedStyle(element, "::before").content) &&
  ["none", "normal"].includes(getComputedStyle(element, "::after").content);

type Escape = "none" | "absolute" | "fixed";
const containingEffects =
  "transform translate rotate scale perspective filter backdrop-filter".split(
    " ",
  );
const establishesContainingBlock = (
  style: Readonly<CSSStyleDeclaration>,
): boolean =>
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
  if (position === "fixed") return "fixed";
  if (position === "absolute") return "absolute";
  return "none";
};
const settledEscape = (
  pending: Escape,
  style: Readonly<CSSStyleDeclaration>,
): Escape => {
  if (pending === "none") return "none";
  if (establishesContainingBlock(style)) return "none";
  if (pending === "absolute" && style.position !== "static") return "none";
  return pending;
};
const clipsOverflow = (style: Readonly<CSSStyleDeclaration>): boolean =>
  style.overflowX !== "visible" || style.overflowY !== "visible";
const clipsPaint = (style: Readonly<CSSStyleDeclaration>): boolean =>
  ["paint", "strict", "content"].some((value) =>
    style.contain.split(/\s+/u).includes(value),
  );
const clips = (style: Readonly<CSSStyleDeclaration>): boolean =>
  clipsOverflow(style) || clipsPaint(style);
const bodyClipsViewport = (
  parent: Readonly<Element>,
  style: Readonly<CSSStyleDeclaration>,
): boolean => {
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
const nearestClipFrom = (
  element: Readonly<Element>,
  escaping: Escape,
): Element | null => {
  const parent = element.parentElement;
  if (parent === null) return null;
  const pending = nextEscape(getComputedStyle(element).position, escaping);
  const style = getComputedStyle(parent);
  const settled = settledEscape(pending, style);
  const root = element.ownerDocument.documentElement;
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
const outsideReach = (
  inner: Readonly<Interval>,
  clip: Readonly<Interval>,
  axis: Readonly<Axis>,
): boolean => {
  if (!axis.canScroll) return inner.start < clip.start || inner.end > clip.end;
  const minimum = axis.reverse ? -axis.extent : 0;
  const maximum = axis.reverse ? 0 : axis.extent;
  return (
    inner.start < clip.start - (axis.offset - minimum) ||
    inner.end > clip.end + (maximum - axis.offset)
  );
};

type ClipBounds = Pick<DOMRectReadOnly, "left" | "right" | "top" | "bottom">;
const reversedFlex = (
  style: Readonly<CSSStyleDeclaration>,
  direction: string,
): boolean =>
  ["flex", "inline-flex"].includes(style.display) &&
  style.flexDirection === direction;
const viewportClipped = (
  parent: Readonly<Element>,
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
export const clipped = (element: Readonly<Element>): boolean => {
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

export const childInside = (
  element: Readonly<Element>,
  child: Readonly<Element>,
): boolean => {
  const inner = child.getBoundingClientRect();
  const outer = element.getBoundingClientRect();
  const left = outer.left + element.clientLeft;
  const top = outer.top + element.clientTop;
  return (
    inner.left >= left &&
    inner.right <= left + element.clientWidth &&
    inner.top >= top &&
    inner.bottom <= top + element.clientHeight
  );
};

const irregularClip = (style: Readonly<CSSStyleDeclaration>): boolean =>
  style.clipPath !== "none" ||
  style.getPropertyValue("clip") !== "auto" ||
  ["paint", "strict", "content"].some((value) =>
    style.contain.split(/\s+/u).includes(value),
  );
const textOutsideClip = (
  parent: Readonly<Element>,
  text: Readonly<DOMRect>,
  style: Readonly<CSSStyleDeclaration>,
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
  parent: Readonly<Element> | null,
  text: Readonly<DOMRect>,
): boolean => {
  if (parent === null) return true;
  const style = getComputedStyle(parent);
  if (irregularClip(style)) return false;
  if (!bodyClipsViewport(parent, style) && textOutsideClip(parent, text, style))
    return false;
  return textInsideClippingAncestors(parent.parentElement, text);
};
const isScreenReaderClip = (element: Readonly<Element>): boolean => {
  const style = getComputedStyle(element);
  return (
    style.position === "absolute" &&
    style.overflowX === "hidden" &&
    style.overflowY === "hidden" &&
    (style.getPropertyValue("clip") === "rect(0px, 0px, 0px, 0px)" ||
      style.clipPath === "inset(50%)")
  );
};
export const isScreenReaderTable = (element: Readonly<Element>): boolean => {
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
