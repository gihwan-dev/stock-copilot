import type { ReactElement } from "react";
import { text } from "../lib";
import { Checkbox } from "./Checkbox";
import type { BaseStory } from "./story-data";
export const checkboxStories: readonly BaseStory[] = (
  ["unchecked", "checked"] as const
).flatMap((checked) =>
  (["enabled", "disabled"] as const).map((disabled): BaseStory => ({
    component: "Checkbox",
    state: disabled === "disabled" ? "disabled" : "normal",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Checkbox
        label={text("app.title", "ko")}
        description={text("ds.description", "ko")}
        defaultChecked={checked}
        disabled={disabled}
      />
    ),
  })),
);
