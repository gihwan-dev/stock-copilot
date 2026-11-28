import { readFileSync } from "node:fs";
import { parse } from "postcss";
const css = readFileSync(
  new URL("../../../design/components/bundle.css", import.meta.url),
  "utf8",
);
export const designUsage = [];
parse(css).walkDecls((d) => {
  for (const m of d.value.matchAll(/var\(--([a-z\d-]+)\)/gu))
    designUsage.push({
      token: m[1],
      property: d.prop,
      selector: d.parent.selector ?? d.parent.name,
    });
});
const prefixes = {
  width: ["w"],
  height: ["h"],
  "min-width": ["min-w"],
  "max-width": ["max-w"],
  "min-height": ["min-h"],
  "max-height": ["max-h"],
  padding: ["p", "px", "py", "pt", "pb", "pl", "pr", "ps", "pe"],
  "padding-left": ["pl"],
  "padding-right": ["pr"],
  "padding-top": ["pt"],
  "padding-bottom": ["pb"],
  margin: ["m", "mx", "my", "mt", "mb", "ml", "mr", "ms", "me"],
  "margin-left": ["ml"],
  "margin-right": ["mr"],
  "margin-top": ["mt"],
  "margin-bottom": ["mb"],
  gap: ["gap", "gap-x", "gap-y"],
  top: ["top"],
  bottom: ["bottom"],
  left: ["left"],
  right: ["right"],
};
export const designLengthClasses = (tokens) => {
  const names = new Set(
    [
      ...tokens.size.tokens,
      ...tokens.spacing.tokens,
      ...tokens.layout.tokens,
    ].map((t) => t.name),
  );
  const entries = designUsage
    .filter((r) => names.has(r.token))
    .flatMap((r) =>
      (prefixes[r.property] ?? []).map((p) => ({
        name: `${p}-${r.token}`,
        token: r.token,
        property: r.property,
      })),
    );
  for (const r of designUsage)
    if (
      r.property === "width" &&
      names.has(r.token) &&
      designUsage.some(
        (s) =>
          s.token === r.token &&
          s.selector === r.selector &&
          s.property === "height",
      )
    )
      entries.push({
        name: `size-${r.token}`,
        token: r.token,
        property: "width/height",
      });
  return [...new Map(entries.map((r) => [r.name, r])).values()].sort((a, b) =>
    a.name.localeCompare(b.name),
  );
};
export const iconUtilities = [
  ["stroke-icon", "stroke-width", "var(--border-width-focus)"],
  ["stroke-linecap-round", "stroke-linecap", "round"],
  ["stroke-linejoin-round", "stroke-linejoin", "round"],
];
const declaration = (property) =>
  property === "width/height" ? ["width", "height"] : [property];
export const designUtilities = (tokens) => [
  ["markdown-bullets", "list-style-type", "disc"],
  ["markdown-bullets", "padding-inline-start", "var(--spacing-space-6)"],
  ["markdown-ordered", "list-style-type", "decimal"],
  ["markdown-ordered", "padding-inline-start", "var(--spacing-space-6)"],
  [
    "markdown-quote",
    "border-inline-start-width",
    "var(--border-width-hairline)",
  ],
  ["markdown-quote", "border-inline-start-style", "solid"],
  ["markdown-quote", "border-inline-start-color", "var(--color-line-strong)"],
  ["markdown-quote", "padding-inline-start", "var(--spacing-space-3)"],
  ["chat-input", "min-height", "var(--spacing-chat-input-min)"],
  ["chat-input", "max-height", "var(--spacing-chat-input-max)"],
  ["chat-input", "resize", "none"],
  ["markdown-content", "min-width", "0"],
  ["markdown-content", "max-width", "var(--spacing-prose-max)"],
  ["markdown-content", "overflow-wrap", "anywhere"],
  ["chat-content", "min-width", "0"],
  ["chat-content", "white-space", "pre-wrap"],
  ["chat-content", "overflow-wrap", "anywhere"],
  ["markdown-link", "white-space", "normal"],
  ["markdown-link", "overflow-wrap", "anywhere"],
  ["markdown-link", "text-decoration-line", "underline"],
  ["status-sentence", "min-width", "0"],
  ["status-sentence", "overflow-wrap", "anywhere"],
  ["dialog-overlay", "position", "fixed"],
  ["dialog-overlay", "inset", "0"],
  ["skeleton-line", "height", "var(--text-size-sm)"],
  ["border-b-hairline", "border-bottom-width", "var(--border-width-hairline)"],
  [
    "transition-disclosure",
    "transition",
    "transform var(--transition-duration-fast) var(--ease-out)",
  ],
  ["rotate-disclosure", "transform", "rotate(90deg)"],
  ["w-switch", "width", "calc(var(--spacing-icon-md) * 2)"],
  ["border-t-hairline", "border-top-width", "var(--border-width-hairline)"],
  ["scroll-viewport-x", "position", "relative"],
  ["scroll-viewport-x", "overflow-x", "auto !important"],
  ["scroll-viewport-x", "overflow-y", "hidden !important"],
  ["scroll-viewport-y", "position", "relative"],
  ["scroll-viewport-y", "overflow-x", "hidden !important"],
  ["scroll-viewport-y", "overflow-y", "auto !important"],
  ["scroll-viewport-both", "position", "relative"],
  ["scroll-viewport-both", "overflow-x", "auto !important"],
  ["scroll-viewport-both", "overflow-y", "auto !important"],
  [
    "scroll-outline-inset",
    "outline-offset",
    "calc(var(--border-width-focus) * -1)",
  ],
  ["scroll-track-x", "height", "var(--spacing-space-2)"],
  ["scroll-track-y", "width", "var(--spacing-space-2)"],
  ["scroll-content-y", "min-width", "0 !important"],
  ["scroll-content-y", "overflow-x", "clip"],
  ["scroll-flex", "min-height", "0"],
  ["scroll-flex", "min-width", "0"],
  ["wrap-slot", "min-width", "min(100%, var(--spacing-popover-min-width))"],
  ...iconUtilities,
  ...designLengthClasses(tokens)
    .filter((r) => r.token.startsWith("border-") || r.token === "focus-offset")
    .flatMap((r) =>
      declaration(r.property).map((p) => [
        r.name,
        p,
        `var(${r.token === "focus-offset" ? "--sc-focus-offset" : `--border-width-${r.token.slice(7)}`})`,
      ]),
    ),
];
const dockTheme = `[data-dock-workspace] {
  --flexlayout-font-family: var(--font-sans);
  --flexlayout-font-size: var(--text-size-md);
  --flexlayout-font-weight: var(--font-weight-medium);
  --flexlayout-color-text: var(--color-ink);
  --flexlayout-color-background: var(--color-surface);
  --flexlayout-color-base: var(--color-surface);
  --flexlayout-color-1: var(--color-bg);
  --flexlayout-color-2: var(--color-surface-sunken);
  --flexlayout-color-3: var(--color-line);
  --flexlayout-color-4: var(--color-line);
  --flexlayout-color-5: var(--color-line-strong);
  --flexlayout-color-6: var(--color-line);
  --flexlayout-color-icon: var(--color-ink-muted);
  --flexlayout-color-overflow: var(--color-ink-muted);
  --flexlayout-color-focus: var(--color-focus-ring);
  --flexlayout-color-tab-selected: var(--color-primary);
  --flexlayout-color-tab-selected-background: var(--color-primary-soft);
  --flexlayout-color-tab-unselected: var(--color-ink-muted);
  --flexlayout-color-drag1: var(--color-primary);
  --flexlayout-color-drag2: var(--color-primary);
  --flexlayout-color-drag1-background: var(--color-primary-soft);
  --flexlayout-color-drag2-background: var(--color-primary-soft);
  --flexlayout-color-drag-rect-border: var(--color-primary);
  --flexlayout-color-drag-rect-background: var(--color-surface-raised);
  --flexlayout-color-edge-icon: var(--color-primary);
  --flexlayout-color-edge-marker: var(--color-line-strong);
  --flexlayout-color-splitter: var(--color-line);
  --flexlayout-color-splitter-hover: var(--color-line-strong);
  --flexlayout-color-splitter-drag: var(--color-primary);
  --flexlayout-color-splitter-handle: var(--color-line-strong);
  --flexlayout-splitter-handle-visibility: visible;
  --flexlayout-splitter-size: var(--border-width-hairline);
  --flexlayout-splitter-active-size: var(--spacing-space-3);
}

[data-widget-gap="4"] [data-dock-workspace] {
  --flexlayout-splitter-size: var(--spacing-space-1);
}

[data-widget-gap="8"] [data-dock-workspace] {
  --flexlayout-splitter-size: var(--spacing-space-2);
}

[data-widget-gap="12"] [data-dock-workspace] {
  --flexlayout-splitter-size: var(--spacing-space-3);
}

[data-widget-radius="4"] .flexlayout__tabset {
  border-radius: var(--radius-sm);
}

[data-widget-radius="8"] .flexlayout__tabset {
  border-radius: var(--radius-md);
}

[data-widget-radius="12"] .flexlayout__tabset {
  border-radius: var(--radius-lg);
}

[data-dock-workspace] .flexlayout__float_window {
  background: var(--color-surface);
  border: var(--border-width-hairline) solid var(--color-line-strong);
  box-shadow: var(--shadow-popover);
}

[data-dock-workspace] .flexlayout__float_window_content {
  margin: 0;
}

[data-dock-workspace] .flexlayout__float_window_drag_handle {
  font-size: var(--spacing-icon-md);
  opacity: 1;
  --fl-color-icon: var(--color-ink);
}

[data-dock-workspace] .flexlayout__float_window_drag_handle + div > div {
  opacity: 1 !important;
}

[data-dock-workspace] .flexlayout__float_window_drag_handle + div > div > div {
  background-color: var(--color-ink-muted) !important;
}

[data-dock-workspace]
  .flexlayout__float_window:not(
    :has(
      .flexlayout__tab_button ~ .flexlayout__tab_button,
      .flexlayout__tabset ~ .flexlayout__tabset
    )
  )
  .flexlayout__tab_toolbar_button-float {
  display: none;
}

[data-dock-workspace] .flexlayout__splitter_drag {
  outline: var(--border-width-hairline) solid var(--color-primary);
}

[data-dock-workspace] .flexlayout__splitter_drag .flexlayout__splitter_handle {
  flex-shrink: 0;
  background-color: var(--color-primary);
}

[data-dock-workspace]
  .flexlayout__splitter_drag
  .flexlayout__splitter_handle_horz {
  width: var(--spacing-space-2);
}

[data-dock-workspace]
  .flexlayout__splitter_drag
  .flexlayout__splitter_handle_vert {
  height: var(--spacing-space-2);
}`;
export const themeSource = (tokens) =>
  '@import "tailwindcss" source(none);\n@import "./tokens.css";\n@import "flexlayout-react/style/light.css";\n@source "../../";\n@source not "./testing/overflow-geometry.ts";\n\n@layer base {\n  html {\n    font-family: var(--font-sans);\n  }\n}\n\n' +
  dockTheme +
  "\n\n" +
  designUtilities(tokens)
    .map(
      ([name, property, value]) =>
        `@utility ${name} {\n  ${property}: ${value};\n}`,
    )
    .join("\n\n") +
  "\n";
