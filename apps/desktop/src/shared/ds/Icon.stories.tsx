import type { ReactElement } from "react";
import { text } from "../lib";
import { Icon } from "./Icon";
import type { IconName } from "./icon-paths";
import type { SizeToken } from "./tokens";
type Story = {
  readonly name: IconName;
  readonly size: Extract<SizeToken, "icon-sm" | "icon-md" | "icon-lg">;
  readonly appearance: "decorative" | "meaningful";
  readonly themes: readonly ["light", "dark"];
  readonly render: () => ReactElement;
};
const names = [
  "arrow-left",
  "bell",
  "chart-candlestick",
  "chart-pie",
  "check",
  "chevron-down",
  "chevron-left",
  "chevron-right",
  "chevron-up",
  "circle-alert",
  "circle-x",
  "clipboard-paste",
  "clock",
  "copy",
  "ellipsis",
  "external-link",
  "eye-off",
  "eye",
  "file-text",
  "fingerprint",
  "info",
  "lock",
  "message-square",
  "minus",
  "newspaper",
  "panel-left-close",
  "panel-left-open",
  "panel-right-close",
  "panel-right-open",
  "plug",
  "plus",
  "refresh-cw",
  "search",
  "send-horizontal",
  "settings",
  "square",
  "star",
  "sunrise",
  "sunset",
  "target",
  "trending-down",
  "trending-up",
  "triangle-alert",
  "wallet",
  "x",
] as const;
const storiesFor = (name: IconName): readonly Story[] => [
  {
    name,
    size: "icon-sm",
    appearance: "decorative",
    themes: ["light", "dark"],
    render: (): ReactElement => <Icon name={name} size="icon-sm" />,
  },
  {
    name,
    size: "icon-sm",
    appearance: "meaningful",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Icon name={name} size="icon-sm" label={text("app.title", "ko")} />
    ),
  },
  {
    name,
    size: "icon-md",
    appearance: "decorative",
    themes: ["light", "dark"],
    render: (): ReactElement => <Icon name={name} size="icon-md" />,
  },
  {
    name,
    size: "icon-md",
    appearance: "meaningful",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Icon name={name} size="icon-md" label={text("app.title", "ko")} />
    ),
  },
  {
    name,
    size: "icon-lg",
    appearance: "decorative",
    themes: ["light", "dark"],
    render: (): ReactElement => <Icon name={name} size="icon-lg" />,
  },
  {
    name,
    size: "icon-lg",
    appearance: "meaningful",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Icon name={name} size="icon-lg" label={text("app.title", "ko")} />
    ),
  },
];
export const iconStories: readonly Story[] = names.flatMap(storiesFor);
