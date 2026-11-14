import type { ReactElement } from "react";
import { text } from "../lib";
import { Select } from "./Select";
import type { BaseStory } from "./story-data";
import { storyOptions } from "./story-data";
export const selectStories: readonly BaseStory[] = [
  {
    component: "Select",
    state: "normal",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Select label={text("app.title", "ko")} options={storyOptions} />
    ),
  },
  {
    component: "Select",
    state: "hint",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Select
        label={text("app.title", "ko")}
        options={storyOptions}
        hint={text("ds.description", "ko")}
      />
    ),
  },
  {
    component: "Select",
    state: "error",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Select
        label={text("app.title", "ko")}
        options={storyOptions}
        error={text("ds.description", "ko")}
      />
    ),
  },
  {
    component: "Select",
    state: "disabled",
    themes: ["light", "dark"],
    render: (): ReactElement => (
      <Select
        label={text("app.title", "ko")}
        options={storyOptions}
        disabled="disabled"
      />
    ),
  },
];
