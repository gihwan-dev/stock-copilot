import { createElement } from "react";
import {
  baseComponentStories,
  feedbackComponentStories,
  statusComponentStories,
  chartComponentStories,
  numberComponentStories,
  contentComponentStories,
  catalogAdditionalStories,
} from "../src/shared/ds/stories.ts";
import { catalogSamples } from "../../../harness/ds-catalog/samples.mjs";
const samples = catalogSamples({
  baseComponentStories,
  feedbackComponentStories,
  statusComponentStories,
  chartComponentStories,
  numberComponentStories,
  contentComponentStories,
  catalogAdditionalStories,
});
const Example = ({ render, theme }) => render(theme);
export const renderCatalog = (component, theme, sample) => {
  const story = samples.find(
    (story) => story.component === component && story.sample === sample,
  );
  if (story === undefined || (theme !== "light" && theme !== "dark"))
    throw new Error("목록의 컴포넌트·예제·테마가 올바르지 않아요");
  return createElement(
    "div",
    {
      className: "bg-bg text-ink p-space-8",
      "data-catalog-component": component,
      "data-catalog-theme": theme,
      "data-catalog-sample": sample,
    },
    createElement(
      "section",
      {
        key: `${component}-${sample}-${theme}`,
        className: "bg-surface p-space-4",
      },
      createElement(Example, { render: story.render, theme }),
    ),
  );
};
