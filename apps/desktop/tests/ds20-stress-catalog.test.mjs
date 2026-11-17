import { strict as assert } from "node:assert";
import { join } from "node:path";
import { test } from "node:test";
import {
  stressProblems,
  stressRenderProblems,
} from "../../../harness/ds-catalog/rules.mjs";
import { loadCatalog } from "../../../harness/ds-catalog/load.mjs";
import { componentsDom } from "./ds-components-dom.mjs";
import { desktopRoot } from "./paths.mjs";

test("each public component must have its own stress story", () => {
  const components = ["Text", "Button", "Money"];
  const samples = [
    { component: "Text", state: "stress", sample: 1 },
    { component: "Button", state: "normal", sample: 1 },
    { component: "Money", state: "stress", sample: 2 },
  ];
  assert.deepEqual(stressProblems(components, samples), [
    "Button: 부하 이야기 없음",
  ]);
  assert.deepEqual(
    stressProblems(components, [
      ...samples,
      { component: "Button", state: "stress", sample: 3 },
    ]),
    [],
  );
  assert.deepEqual(stressProblems(["Text", "Missing"], samples), [
    "Missing: 부하 이야기 없음",
  ]);
});

test("the actual catalog provides stress stories for all public components", async () => {
  const catalog = await loadCatalog();
  assert.deepEqual(
    stressProblems(Object.keys(catalog.components), catalog.samples),
    [],
  );
});

test("stress stories fail when their render is empty in either theme", async () => {
  const { createElement } = await import("react");
  const samples = [
    { component: "Money", render: () => null },
    {
      component: "Percent",
      render: (theme) => (theme === "dark" ? null : createElement("span")),
    },
    { component: "Quantity", render: () => createElement("span") },
  ];
  assert.deepEqual(stressRenderProblems(samples), [
    "Money: light 부하 이야기 렌더 없음",
    "Money: dark 부하 이야기 렌더 없음",
    "Percent: dark 부하 이야기 렌더 없음",
  ]);
  const catalog = await loadCatalog();
  assert.deepEqual(stressRenderProblems(catalog.protectedStressStories), []);
});

test("the additional ChatInput stress story starts with protected stress content", async () => {
  const catalog = await loadCatalog();
  const chatStories = catalog.samples.filter(
    (story) => story.component === "ChatInput" && story.state === "stress",
  );
  const draft = chatStories.at(-1)?.render().props.initialDraft;
  assert.equal(typeof draft, "string");
  assert.ok(draft.includes("대한미래첨단산업글로벌에너지"));
});

test("ChatInput actually renders the complete protected draft", async () => {
  const bundle = await componentsDom(`
import {render,cleanup} from "@testing-library/react";
import {stressComponentStories} from ${JSON.stringify(join(desktopRoot, "src/shared/ds/testing/Stress.stories.tsx"))};
import {stressFixtures} from ${JSON.stringify(join(desktopRoot, "src/shared/ds/testing/stress.ts"))};
window.cleanup=cleanup;
window.expectedDraft=[stressFixtures.company,...stressFixtures.paragraphs].join("\\n");
window.renderChat=()=>render(stressComponentStories.find(story=>story.component==="ChatInput").render("light"));
`);
  try {
    const window = bundle.open();
    window.renderChat();
    assert.equal(
      window.document.querySelector("textarea")?.value,
      window.expectedDraft,
    );
  } finally {
    bundle.close();
  }
});

test("stress stories exercise long digits and an empty value", async () => {
  const catalog = await loadCatalog();
  const input = catalog.samples.find(
    (story) => story.component === "TextField" && story.state === "stress",
  );
  const empty = catalog.samples.filter(
    (story) => story.component === "EmptyState" && story.state === "stress",
  );
  assert.equal(
    input?.render().props.defaultValue,
    "12345678901234567890123456789012345678901234567890",
  );
  assert.ok(empty.some((story) => story.render().props.title === ""));
});

test("trend chart stress stories render charts with distinct points", async () => {
  const { renderToStaticMarkup } = await import("react-dom/server");
  const catalog = await loadCatalog();
  for (const component of ["LineChart", "AreaChart"]) {
    const story = catalog.samples
      .filter((item) => item.component === component && item.state === "stress")
      .at(-1);
    assert.ok(story, component);
    const element = story.render("light");
    assert.ok(element.props.data.length >= 2, component);
    assert.equal(
      new Set(element.props.data.map((point) => point.id)).size,
      element.props.data.length,
      component,
    );
    assert.match(renderToStaticMarkup(element), /<svg\b/u, component);
  }
});

test("Text has a stress story with an unbroken English word", async () => {
  const catalog = await loadCatalog();
  const stories = catalog.samples.filter(
    (item) => item.component === "Text" && item.state === "stress",
  );
  assert.ok(
    stories.some((story) =>
      String(story.render().props.children).includes(
        "ExtraordinarilyLongUnbrokenInternationalSemiconductorInvestmentCompanyName",
      ),
    ),
  );
});

test("Percent stresses long numeric output while Probability stays in range", async () => {
  const { renderToStaticMarkup } = await import("react-dom/server");
  const catalog = await loadCatalog();
  const content = (component) => {
    const story = catalog.protectedStressStories.find(
      (item) => item.component === component,
    );
    assert.ok(story, component);
    return renderToStaticMarkup(story.render("light"));
  };
  assert.match(content("Percent"), /9,876,543,210,123%/u);
  assert.match(content("Probability"), /99%/u);
});

test("components without free-text props include protected context in stress stories", async () => {
  const { renderToStaticMarkup } = await import("react-dom/server");
  const catalog = await loadCatalog();
  for (const component of ["Skeleton", "DirectionBadge"]) {
    const story = catalog.protectedStressStories.find(
      (item) => item.component === component,
    );
    assert.ok(story, component);
    const markup = renderToStaticMarkup(story.render("light"));
    assert.match(markup, /대한미래첨단산업글로벌에너지/u, component);
  }
});

test("a legacy stress state cannot replace the protected ChatInput story", async () => {
  const catalog = await loadCatalog();
  assert.ok(
    catalog.samples.some(
      (story) => story.component === "ChatInput" && story.state === "stress",
    ),
  );
  assert.deepEqual(
    stressProblems(
      ["ChatInput"],
      catalog.protectedStressStories.filter(
        (story) => story.component !== "ChatInput",
      ),
    ),
    ["ChatInput: 부하 이야기 없음"],
  );
});
