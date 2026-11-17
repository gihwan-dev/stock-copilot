import { strict as assert } from "node:assert";
import { test } from "node:test";
import { join } from "node:path";
import { componentsDom } from "./ds-components-dom.mjs";
import { desktopRoot } from "./paths.mjs";

test("overflow inspection rejects a document without layout", async () => {
  const bundle = await componentsDom(`
import {inspectOverflow} from ${JSON.stringify(join(desktopRoot, "src/shared/ds/testing/overflow.ts"))};
window.inspectOverflow=inspectOverflow;
window.cleanup=()=>{};
`);
  try {
    const window = bundle.open();
    window.document.body.innerHTML =
      '<div style="width:1px">long content</div>';
    assert.deepEqual(
      Array.from(
        window.inspectOverflow(window.document),
        ({ kind, element }) => ({
          kind,
          element,
        }),
      ),
      [{ kind: "layout-unavailable", element: "html" }],
    );
  } finally {
    bundle.close();
  }
});
