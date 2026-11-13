export type ColorToken =
  | "bg"
  | "surface"
  | "surface-raised"
  | "surface-sunken"
  | "line"
  | "line-strong"
  | "ink"
  | "ink-muted"
  | "ink-subtle"
  | "primary"
  | "primary-hover"
  | "primary-soft"
  | "on-primary"
  | "up"
  | "up-soft"
  | "on-up"
  | "up-hover"
  | "down"
  | "down-soft"
  | "on-down"
  | "down-hover"
  | "alert"
  | "alert-soft"
  | "on-alert"
  | "focus-ring"
  | "chart-1"
  | "chart-2"
  | "chart-3"
  | "chart-4"
  | "chart-5"
  | "chart-6"
  | "overlay";

export type SpacingToken =
  | "space-1"
  | "space-2"
  | "space-3"
  | "space-4"
  | "space-5"
  | "space-6"
  | "space-7"
  | "space-8"
  | "space-9"
  | "space-10"
  | "space-11"
  | "space-12";

export type RadiusToken =
  "radius-sm" | "radius-md" | "radius-lg" | "radius-pill";

export type ShadowToken = "shadow-popover";

export type SizeToken =
  | "sidebar-width"
  | "sidebar-collapsed"
  | "chat-width"
  | "content-min"
  | "row-height"
  | "row-height-dense"
  | "control-height"
  | "control-height-sm"
  | "control-pad-sm"
  | "chat-width-min"
  | "chat-width-max"
  | "toolbar-height"
  | "list-item-height"
  | "icon-sm"
  | "icon-md"
  | "icon-lg"
  | "dot-size"
  | "bar-height"
  | "meter-height"
  | "gap-hairline"
  | "border-hairline"
  | "border-mark"
  | "border-focus"
  | "focus-offset"
  | "dialog-width"
  | "dialog-width-wide"
  | "popover-min-width"
  | "tooltip-max-width"
  | "chart-height-sm"
  | "chart-height"
  | "chart-height-lg"
  | "chat-input-min"
  | "chat-input-max"
  | "bubble-max-width";

export type DurationToken =
  "duration-fast" | "duration-base" | "duration-flash";

export type EasingToken = "ease-out" | "ease-in";

export type ZIndexToken =
  "z-sticky" | "z-panel" | "z-popover" | "z-modal" | "z-toast";

export type OpacityToken = "opacity-disabled" | "opacity-area";

export type LayoutToken =
  | "window-min-width"
  | "window-min-height"
  | "bp-window-md"
  | "bp-window-lg"
  | "bp-content-medium"
  | "bp-content-wide"
  | "content-max"
  | "prose-max"
  | "grid-columns";

export type TextToken =
  | "text-xs"
  | "text-sm"
  | "text-md"
  | "text-base"
  | "text-lg"
  | "text-xl"
  | "text-2xl"
  | "text-3xl"
  | "leading-xs"
  | "leading-sm"
  | "leading-md"
  | "leading-base"
  | "leading-lg"
  | "leading-xl"
  | "leading-3xl"
  | "weight-regular"
  | "weight-medium"
  | "weight-semibold"
  | "weight-bold"
  | "tracking-display";

export type TypeStyleToken =
  | "num-display"
  | "num-lg"
  | "num-md"
  | "num-sm"
  | "title-lg"
  | "title-md"
  | "title-sm"
  | "body"
  | "body-sm"
  | "label"
  | "caption"
  | "code";

export type FontToken = "sans" | "mono";

export type TokenName =
  | ColorToken
  | SpacingToken
  | RadiusToken
  | ShadowToken
  | SizeToken
  | DurationToken
  | EasingToken
  | ZIndexToken
  | OpacityToken
  | LayoutToken
  | TextToken
  | TypeStyleToken
  | FontToken;
