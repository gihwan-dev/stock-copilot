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
import { stressComponentStories } from "../src/shared/ds/testing/Stress.stories.tsx";
import { ScrollArea } from "../src/shared/ds/ScrollArea.tsx";
import { catalogSamples } from "../../../harness/ds-catalog/samples.mjs";
const samples = catalogSamples({
  baseComponentStories,
  feedbackComponentStories,
  statusComponentStories,
  chartComponentStories,
  numberComponentStories,
  contentComponentStories,
  catalogAdditionalStories,
  stressComponentStories,
});
const numericComponents = new Set([
  "Money",
  "Percent",
  "Quantity",
  "Probability",
  "PriceChange",
]);
const Example = ({ render, theme }) => render(theme);
const Stage = ({ component, render, theme }) =>
  createElement(
    ScrollArea,
    { direction: "vertical", layout: "stage" },
    numericComponents.has(component)
      ? createElement(
          ScrollArea,
          { direction: "horizontal", layout: "numeric" },
          createElement(
            "div",
            { "data-catalog-content": "", className: "w-max" },
            createElement(Example, { render, theme }),
          ),
        )
      : createElement(
          "div",
          { "data-catalog-content": "", className: "w-full scroll-flex" },
          createElement(Example, { render, theme }),
        ),
  );
export const renderCatalog = (component, theme, sample) => {
  const story = samples.find(
    (story) => story.component === component && story.sample === sample,
  );
  if (story === undefined || (theme !== "light" && theme !== "dark"))
    throw new Error("목록의 컴포넌트·예제·테마가 올바르지 않아요");
  return createElement(
    "div",
    {
      className: "h-full flex flex-col bg-surface text-ink p-space-4",
      "data-catalog-component": component,
      "data-catalog-theme": theme,
      "data-catalog-sample": sample,
    },
    createElement(
      "section",
      {
        key: `${component}-${sample}-${theme}`,
        className: "flex-1 scroll-flex flex flex-col bg-surface px-space-4",
      },
      createElement(Stage, { component, render: story.render, theme }),
    ),
  );
};
