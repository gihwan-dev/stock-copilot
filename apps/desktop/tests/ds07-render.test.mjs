import assert from "node:assert/strict";
import test from "node:test";
import { componentsDom } from "./ds-components-dom.mjs";
import { desktopRoot } from "./paths.mjs";
const catalog = JSON.stringify(`${desktopRoot}/.storybook/catalog.mjs`);
const preview = JSON.stringify(`${desktopRoot}/.storybook/preview.mjs`);
test("DS07 catalog renders every public component in each real theme", async () => {
  const bundle =
    await componentsDom(`import {render,cleanup} from "@testing-library/react";
import {renderCatalog} from ${catalog};
import * as sources from STORY_ENTRY;
const catalogStories=Object.values(sources).flat();
import preview from ${preview};
window.cleanup=cleanup;window.samples=catalogStories;
window.show=(component,theme)=>render(preview.decorators[0](()=>renderCatalog(component,theme),{parameters:{theme}}));`);
  try {
    const w = bundle.open();
    const names = [...new Set(Array.from(w.samples, (s) => s.component))];
    assert.equal(names.length, 30);
    for (const name of names)
      for (const theme of ["light", "dark"]) {
        const shown = w.show(name, theme);
        assert.equal(w.document.documentElement.dataset.theme, theme);
        assert.equal(
          shown.container.firstElementChild.children.length,
          Array.from(w.samples).filter((s) => s.component === name).length,
        );
        for (const chart of shown.container.querySelectorAll(
          "[data-chart-theme]",
        ))
          assert.equal(chart.dataset.chartTheme, theme);
        w.cleanup();
      }
  } finally {
    bundle.close();
  }
});
