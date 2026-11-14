import type { ReactElement } from "react";
import { text } from "../lib";
import { Button } from "./Button";
import type { BaseStory } from "./story-data";
export const buttonStories: readonly BaseStory[] = (
  ["primary", "secondary", "ghost", "buy", "sell"] as const
).flatMap((variant) =>
  (["control-height", "control-height-sm"] as const).flatMap((size) =>
    (["enabled", "disabled"] as const).map((disabled): BaseStory => ({
      component: "Button",
      state: disabled === "disabled" ? "disabled" : "normal",
      themes: ["light", "dark"],
      render: (): ReactElement => (
        <Button variant={variant} size={size} disabled={disabled}>
          {text("app.title", "ko")}
        </Button>
      ),
    })),
  ),
);
