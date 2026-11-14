import type { ReactElement } from "react";
import { text } from "../lib";
import { TextField } from "./TextField";
import type { BaseStory } from "./story-data";
import { storyValue } from "./story-data";
export const textFieldStories: readonly BaseStory[] = [
  {
    component: "TextField",
    state: "normal",
    themes: ["light", "dark"],
    render: (): ReactElement | null => (
      <TextField label={text("app.title", "ko")} />
    ),
  },
  {
    component: "TextField",
    state: "hint",
    themes: ["light", "dark"],
    render: (): ReactElement | null => (
      <TextField
        label={text("app.title", "ko")}
        hint={text("ds.description", "ko")}
      />
    ),
  },
  {
    component: "TextField",
    state: "error",
    themes: ["light", "dark"],
    render: (): ReactElement | null => (
      <TextField
        label={text("app.title", "ko")}
        error={text("ds.description", "ko")}
      />
    ),
  },
  {
    component: "TextField",
    state: "disabled",
    themes: ["light", "dark"],
    render: (): ReactElement | null => (
      <TextField label={text("app.title", "ko")} disabled="disabled" />
    ),
  },
  {
    component: "TextField",
    state: "secret",
    themes: ["light", "dark"],
    render: (): ReactElement | null => (
      <TextField
        label={text("app.title", "ko")}
        type="secret"
        defaultValue={storyValue}
      />
    ),
  },
  {
    component: "TextField",
    state: "empty",
    themes: ["light", "dark"],
    render: (): ReactElement | null => (
      <TextField label={text("app.title", "ko")} type="secret" />
    ),
  },
];
