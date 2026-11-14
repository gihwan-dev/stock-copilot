import type { ReactElement } from "react";
import { text } from "../lib";
import { IconButton } from "./IconButton";
import type { BaseStory } from "./story-data";
import type { ControlSize } from "./control-types";
const states = (size: ControlSize): readonly BaseStory[] => [
  {
    component: "IconButton",
    state: "normal",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <IconButton icon="settings" label={text("app.title", "ko")} size={size} />
    ),
  },
  {
    component: "IconButton",
    state: "selected",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <IconButton
        icon="settings"
        label={text("app.title", "ko")}
        size={size}
        pressed="pressed"
      />
    ),
  },
  {
    component: "IconButton",
    state: "open",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <IconButton
        icon="settings"
        label={text("app.title", "ko")}
        size={size}
        expanded="open"
      />
    ),
  },
  {
    component: "IconButton",
    state: "disabled",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <IconButton
        icon="settings"
        label={text("app.title", "ko")}
        size={size}
        disabled="disabled"
      />
    ),
  },
];
const tooltips = (size: ControlSize): readonly BaseStory[] => [
  {
    component: "IconButton",
    state: "normal",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <IconButton
        icon="settings"
        label={text("app.title", "ko")}
        size={size}
        tooltipSide="right"
      />
    ),
  },
  {
    component: "IconButton",
    state: "normal",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <IconButton
        icon="settings"
        label={text("app.title", "ko")}
        size={size}
        tooltip="hidden"
      />
    ),
  },
];
export const iconButtonStories: readonly BaseStory[] = (
  ["control-height", "control-height-sm"] as const
).flatMap((size) => [...states(size), ...tooltips(size)]);
