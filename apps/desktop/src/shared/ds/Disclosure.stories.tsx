import type { ReactElement } from "react";
import { text } from "../lib";
import { Disclosure } from "./Disclosure";
import type { BaseStory } from "./story-data";
import { Text } from "./Text";
export const disclosureStories: readonly BaseStory[] = (
  ["closed", "open"] as const
).map((open): BaseStory => ({
  component: "Disclosure",
  state: open,
  themes: ["light", "dark"],
  render: (): ReactElement => (
    <Disclosure title={text("app.title", "ko")} defaultOpen={open}>
      <Text>{text("ds.description", "ko")}</Text>
    </Disclosure>
  ),
}));
