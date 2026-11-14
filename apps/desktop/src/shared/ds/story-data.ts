import type { ReactElement } from "react";
import {
  parseInputValue,
  emptyInputValue,
  text,
  type InputValue,
} from "../lib";
import type { Segments } from "./control-types";
export type BaseStory = {
  readonly component:
    | "Text"
    | "Heading"
    | "Button"
    | "IconButton"
    | "TextField"
    | "Select"
    | "Checkbox"
    | "Switch"
    | "SegmentedControl"
    | "Disclosure"
    | "Icon";
  readonly state:
    | "normal"
    | "disabled"
    | "selected"
    | "error"
    | "hint"
    | "secret"
    | "empty"
    | "open"
    | "closed"
    | "decorative"
    | "meaningful";
  readonly themes: readonly ["light", "dark"];
  readonly render: () => ReactElement | null;
};
const value = (raw: "day" | "month" | "year" | "example-input"): InputValue => {
  const parsed = parseInputValue(raw);
  return parsed.kind === "ok" ? parsed.value : emptyInputValue;
};
export const storyValue = value("example-input");
export const storyOptions: Segments = [
  { value: value("day"), label: text("ds.day", "ko") },
  { value: value("month"), label: text("ds.month", "ko") },
  { value: value("year"), label: text("ds.year", "ko") },
];
