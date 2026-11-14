import type { ReactElement } from "react";
import { text } from "../lib";
import { SegmentedControl } from "./SegmentedControl";
import type { BaseStory } from "./story-data";
import { storyOptions } from "./story-data";
export const segmentedStories: readonly BaseStory[] = storyOptions.map(
  (option): BaseStory => ({
    component: "SegmentedControl",
    state: "selected",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <SegmentedControl
        label={text("app.title", "ko")}
        options={storyOptions}
        defaultValue={option.value}
      />
    ),
  }),
);
