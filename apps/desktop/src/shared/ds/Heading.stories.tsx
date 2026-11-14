import type { ReactElement } from "react";
import { text } from "../lib";
import { Heading } from "./Heading";
import type { BaseStory } from "./story-data";
export const headingStories: readonly BaseStory[] = ([1, 2, 3] as const).map(
  (level): BaseStory => ({
    component: "Heading",
    state: "normal",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Heading level={level}>{text("app.title", "ko")}</Heading>
    ),
  }),
);
