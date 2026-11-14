import type { ReactElement } from "react";
import { text } from "../lib";
import { Text } from "./Text";
import type { BaseStory } from "./story-data";
export const textStories: readonly BaseStory[] = (
  ["body", "body-sm", "label", "caption", "code"] as const
).flatMap((variant) =>
  (["ink", "ink-muted", "ink-subtle", "primary", "alert"] as const).flatMap(
    (tone) =>
      (["p", "span", "div"] as const).map((as): BaseStory => ({
        component: "Text",
        state: "normal",
        themes: ["light", "dark"],
        render: (): ReactElement => (
          <Text variant={variant} tone={tone} as={as}>
            {text("app.title", "ko")}
          </Text>
        ),
      })),
  ),
);
