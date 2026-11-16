import assert from "node:assert/strict";
import test from "node:test";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { componentsDom } from "./ds-components-dom.mjs";
import { desktopRoot } from "./paths.mjs";
const files = readdirSync(join(desktopRoot, ".storybook/catalog")).filter(
  (file) => file.endsWith(".stories.mjs"),
);
const imports = files
  .map(
    (file, index) =>
      `import * as page${index} from ${JSON.stringify(join(desktopRoot, ".storybook/catalog", file))};`,
  )
  .join("\n");
const cardIndex = files.indexOf("Card.stories.mjs");
const assertTheme = (document, theme) =>
  assert.equal(document.documentElement.dataset.theme, theme);
const pages = files
  .map(
    (file, index) =>
      `...Object.values(composeStories(page${index},preview)).map(page=>({component:${JSON.stringify(file.replace(".stories.mjs", ""))},page}))`,
  )
  .join(",");
const summary = (scope, document) => ({
  text: document.body.textContent,
  nodes: Array.from(scope.querySelectorAll("*"), (element) => [
    element.tagName,
    ...[
      "class",
      "role",
      "type",
      "aria-label",
      "aria-expanded",
      "aria-checked",
      "aria-disabled",
      "disabled",
      "data-chart-theme",
      "d",
    ].map((name) => element.getAttribute(name)),
  ]),
  values: Array.from(
    scope.querySelectorAll("input,textarea,select"),
    (element) => element.value,
  ),
  dialogs: Array.from(
    document.querySelectorAll('[role="dialog"]'),
    (element) => element.textContent,
  ),
});
const tick = (window) =>
  new Promise((resolve) => window.setTimeout(resolve, 0));
test("DS07 every actual CSF render uses its component, example and real theme", async () => {
  const originalBundle =
    await componentsDom(`import {render,cleanup} from "@testing-library/react";
import * as sources from STORY_ENTRY;
window.cleanup=cleanup;
window.findSource=(component,sample)=>Object.keys(sources).sort().flatMap(key=>sources[key]).filter(story=>story.component===component && (!("theme" in story)||story.theme==="light"))[sample];
window.expected=(story,theme)=>{document.documentElement.dataset.theme=theme;return render(story.render(theme));};`);
  let bundle;
  try {
    bundle =
      await componentsDom(`import {render,cleanup} from "@testing-library/react";
import {composeStories} from "@storybook/react-vite";
import * as components from DS_ENTRY;
import preview from ${JSON.stringify(join(desktopRoot, ".storybook/preview.mjs"))};
${imports}
window.cleanup=cleanup;window.names=Object.keys(components);window.pages=[${pages}];window.show=page=>render(page());
const broken=composeStories({...page${cardIndex},default:{...page${cardIndex}.default,decorators:[Story=>{document.documentElement.dataset.theme="light";return Story();}]}},preview);
window.showBroken=()=>render(broken.Dark0());
window.invalidTheme=theme=>preview.decorators[0](()=>null,{parameters:{theme}});`);
    const original = originalBundle.open(),
      w = bundle.open();
    assert.deepEqual(
      [...new Set(Array.from(w.pages, (entry) => entry.component))].sort(),
      Array.from(w.names).sort(),
    );
    for (const { component, page } of w.pages) {
      const source = original.findSource(component, page.parameters.sample);
      assert.ok(source);
      const expected = original.expected(source, page.parameters.theme);
      await tick(original);
      const expectedSummary = summary(expected.container, original.document);
      original.cleanup();
      const shown = w.show(page);
      await tick(w);
      const root = shown.container.firstElementChild;
      assertTheme(w.document, page.parameters.theme);
      assert.ok(w.document.documentElement.classList.contains("bg-surface"));
      assert.equal(root.dataset.catalogComponent, component);
      assert.equal(root.dataset.catalogTheme, page.parameters.theme);
      assert.equal(
        root.dataset.catalogSample,
        page.parameters.sample.toString(),
      );
      assert.equal(root.children.length, 1);
      assert.deepEqual(
        summary(root.firstElementChild, w.document),
        expectedSummary,
        `${component}/${page.parameters.sample}/${page.parameters.theme}`,
      );
      assert.ok(w.document.querySelectorAll('[role="dialog"]').length <= 1);
      for (const chart of shown.container.querySelectorAll(
        "[data-chart-theme]",
      ))
        assert.equal(chart.dataset.chartTheme, page.parameters.theme);
      if (component === "Table" && source.state === "long") {
        assert.ok(shown.container.textContent.includes("987,654,321,098"));
        assert.ok(
          shown.container.textContent.includes(
            "예시반도체장기성장산업기술연구개발통합솔루션보통주",
          ),
        );
      }
      if (source.state === "representative") {
        assert.equal(
          shown.container.querySelectorAll("[data-chart-theme]").length,
          1,
        );
        const trend = component === "LineChart" || component === "AreaChart";
        const names = trend
          ? ["11월 11일", "11월 12일", "11월 13일"]
          : [
              "예시반도체장기성장산업기술연구개발통합솔루션보통주",
              "예시생활건강",
              "예시미래에너지",
            ];
        const values = trend
          ? ["24,500,000,000", "24,600,000,000", "24,700,000,000"]
          : ["24,500,000,000", "18,600,000,000", "32,700,000,000"];
        for (const value of [...names, ...values])
          assert.ok(
            w.document.body.textContent.includes(value),
            `${component}: ${value}`,
          );
      }
      if (source.state === "long" && component === "Text")
        assert.ok(shown.container.textContent.length >= 80);
      w.cleanup();
    }
    w.showBroken();
    await tick(w);
    assert.throws(() => assertTheme(w.document, "dark"));
    w.cleanup();
    assert.throws(() => w.invalidTheme(undefined));
    assert.throws(() => w.invalidTheme("purple"));
  } finally {
    bundle?.close();
    originalBundle.close();
  }
});
