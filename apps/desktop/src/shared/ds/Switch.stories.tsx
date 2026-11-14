import type { ReactElement } from "react";
import { text } from "../lib";
import { Switch } from "./Switch";
import type { BaseStory } from "./story-data";
export const switchStories: readonly BaseStory[] = (
  ["off", "on"] as const
).flatMap((checked) =>
  (["enabled", "disabled"] as const).map((disabled): BaseStory => ({
    component: "Switch",
    state: disabled === "disabled" ? "disabled" : "normal",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Switch
        label={text("app.title", "ko")}
        description={text("ds.description", "ko")}
        defaultChecked={checked}
        disabled={disabled}
      />
    ),
  })),
);
