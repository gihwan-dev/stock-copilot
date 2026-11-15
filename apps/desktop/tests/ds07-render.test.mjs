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
const pages = files
  .map(
    (file, index) =>
      `...Object.entries(page${index}).filter(([name])=>name!=="default").map(([,page])=>({component:${JSON.stringify(file.replace(".stories.mjs", ""))},page}))`,
  )
  .join(",");
test("DS07 every actual CSF render uses its component, example and real theme", async () => {
  const bundle =
    await componentsDom(`import {render,cleanup} from "@testing-library/react";
import * as components from DS_ENTRY;
import preview from ${JSON.stringify(join(desktopRoot, ".storybook/preview.mjs"))};
${imports}
window.cleanup=cleanup;window.names=Object.keys(components);window.pages=[${pages}];
window.show=page=>render(preview.decorators[0](()=>page.render(),{parameters:page.parameters}));
window.invalidTheme=theme=>preview.decorators[0](()=>null,{parameters:{theme}});`);
  try {
    const w = bundle.open();
    assert.deepEqual(
      [...new Set(Array.from(w.pages, (entry) => entry.component))].sort(),
      Array.from(w.names).sort(),
    );
    for (const { component, page } of w.pages) {
      const shown = w.show(page);
      const root = shown.container.firstElementChild;
      assert.equal(
        w.document.documentElement.dataset.theme,
        page.parameters.theme,
      );
      assert.equal(root.dataset.catalogComponent, component);
      assert.equal(root.dataset.catalogTheme, page.parameters.theme);
      assert.equal(
        root.dataset.catalogSample,
        page.parameters.sample.toString(),
      );
      assert.equal(root.children.length, 1);
      assert.ok(w.document.querySelectorAll('[role="dialog"]').length <= 1);
      for (const chart of shown.container.querySelectorAll(
        "[data-chart-theme]",
      ))
        assert.equal(chart.dataset.chartTheme, page.parameters.theme);
      if (
        component === "Table" &&
        shown.container.textContent.includes("가상 보유 종목")
      ) {
        assert.ok(shown.container.textContent.includes("987,654,321,098"));
        assert.ok(
          shown.container.textContent.includes(
            "예시반도체장기성장산업기술연구개발통합솔루션보통주",
          ),
        );
      }
      w.cleanup();
    }
    assert.throws(() => w.invalidTheme(undefined));
    assert.throws(() => w.invalidTheme("purple"));
  } finally {
    bundle.close();
  }
});
