import { createElement } from "react";
import {
  baseComponentStories,
  feedbackComponentStories,
  statusComponentStories,
  chartComponentStories,
  numberComponentStories,
  contentComponentStories,
} from "../src/shared/ds/stories.ts";
const catalogStories = [
  ...baseComponentStories,
  ...feedbackComponentStories,
  ...statusComponentStories,
  ...chartComponentStories,
  ...numberComponentStories,
  ...contentComponentStories,
];
export const renderCatalog = (component, theme) =>
  createElement(
    "div",
    { className: "bg-bg text-ink p-space-8" },
    catalogStories
      .filter((story) => story.component === component)
      .map((story, index) =>
        createElement(
          "section",
          {
            key: `${component}-${index}`,
            className: "bg-surface p-space-4 mb-space-6",
          },
          story.render(theme),
        ),
      ),
  );
