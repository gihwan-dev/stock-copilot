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

type Escape = "none" | "absolute" | "fixed";

const establishesContainingBlock = (
  style: Readonly<CSSStyleDeclaration>,
): boolean =>
  [style.transform, style.perspective, style.filter, style.backdropFilter].some(
    (value) => value !== "" && value !== "none",
  ) ||
  ["layout", "paint", "strict", "content"].some((value) =>
    style.contain.split(/\s+/u).includes(value),
  ) ||
  ["transform", "perspective", "filter", "backdrop-filter"].some((value) =>
    style.willChange.split(/\s*,\s*/u).includes(value),
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

const clips = (style: Readonly<CSSStyleDeclaration>): boolean =>
  style.overflowX !== "visible" || style.overflowY !== "visible";

const bodyClipsViewport = (
  parent: Readonly<Element>,
  style: Readonly<CSSStyleDeclaration>,
): boolean => {
  const root = parent.ownerDocument.documentElement;
  if (parent !== parent.ownerDocument.body || !clips(style)) return false;
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

type ClipBounds = {
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
};

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
