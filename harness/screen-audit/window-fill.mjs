export const inspectWindowFill = (doc) => {
  const view = doc.defaultView;
  const width = view.innerWidth;
  const height = view.innerHeight;
  const tolerance = 1;
  const findings = [];
  const add = (kind, detail) => findings.push({ kind, detail });
  const transparent = (color) =>
    color === "transparent" ||
    /^rgba\([^)]*,\s*0(?:\.0+)?\)$/u.test(color) ||
    /\/\s*0(?:\.0+)?\)$/u.test(color);
  const inside = (rect) =>
    rect.width > 0 &&
    rect.height > 0 &&
    rect.top >= -tolerance &&
    rect.left >= -tolerance &&
    rect.bottom <= height + tolerance &&
    rect.right <= width + tolerance;

  const frame = doc.querySelector("[data-app-frame]");
  if (frame === null) add("frame-missing", "");
  else {
    const style = view.getComputedStyle(frame);
    if (transparent(style.backgroundColor) && style.backgroundImage === "none")
      add("frame-unpainted", style.backgroundColor);
    const rect = frame.getBoundingClientRect();
    if (
      Math.abs(rect.left) > tolerance ||
      Math.abs(rect.top) > tolerance ||
      Math.abs(rect.width - width) > tolerance ||
      Math.abs(rect.height - height) > tolerance
    )
      add(
        "frame-gap",
        `${rect.left},${rect.top} ${rect.width}x${rect.height} / ${width}x${height}`,
      );
  }

  const root = doc.scrollingElement ?? doc.documentElement;
  if (root.scrollWidth > root.clientWidth + tolerance)
    add("document-scroll-x", `${root.scrollWidth}/${root.clientWidth}`);
  if (root.scrollHeight > root.clientHeight + tolerance)
    add("document-scroll-y", `${root.scrollHeight}/${root.clientHeight}`);

  for (const viewport of doc.querySelectorAll("[data-scroll-viewport]")) {
    const style = view.getComputedStyle(viewport);
    const clipsX = style.overflowX === "hidden" || style.overflowX === "clip";
    if (clipsX && viewport.scrollWidth > viewport.clientWidth + tolerance)
      add(
        "clipped-width",
        `${viewport.scrollWidth}/${viewport.clientWidth} ${(viewport.textContent ?? "").slice(0, 40)}`,
      );
  }

  for (const button of doc.querySelectorAll("[data-pinned-actions] button")) {
    const rect = button.getBoundingClientRect();
    const visible = inside(rect);
    const hit = visible
      ? doc.elementFromPoint(
          rect.left + rect.width / 2,
          rect.top + rect.height / 2,
        )
      : null;
    if (!visible || hit === null || !button.contains(hit))
      add(
        "pinned-action-hidden",
        `${button.textContent} ${Math.round(rect.top)}-${Math.round(rect.bottom)}/${height}`,
      );
  }
  for (const dialog of doc.querySelectorAll('[role="dialog"]')) {
    const rect = dialog.getBoundingClientRect();
    if (!inside(rect))
      add(
        "dialog-outside",
        `${Math.round(rect.top)}-${Math.round(rect.bottom)}/${height}`,
      );
  }
  return findings;
};
