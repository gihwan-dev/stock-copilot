import type { BaseStory } from "./story-data";
import { iconStories } from "./Icon.stories";
import { textStories } from "./Text.stories";
import { headingStories } from "./Heading.stories";
import { buttonStories } from "./Button.stories";
import { iconButtonStories } from "./IconButton.stories";
import { textFieldStories } from "./TextField.stories";
import { selectStories } from "./Select.stories";
import { checkboxStories } from "./Checkbox.stories";
import { switchStories } from "./Switch.stories";
import { disclosureStories } from "./Disclosure.stories";
import { segmentedStories } from "./SegmentedControl.stories";
export const baseComponentStories: readonly BaseStory[] = [
  ...iconStories.map((story): BaseStory => ({
    component: "Icon",
    state: story.appearance,
    themes: story.themes,
    render: story.render,
  })),
  ...textStories,
  ...headingStories,
  ...buttonStories,
  ...iconButtonStories,
  ...textFieldStories,
  ...selectStories,
  ...checkboxStories,
  ...switchStories,
  ...disclosureStories,
  ...segmentedStories,
];
