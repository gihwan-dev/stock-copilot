import assert from "node:assert/strict";
import { after, before, test } from "node:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import { ESLint } from "eslint";
import { componentsDom } from "./ds-components-dom.mjs";
import { dsConfigs } from "../eslint/ds-config.mjs";
import { desktopRoot } from "./paths.mjs";

const require = createRequire(join(desktopRoot, "package.json"));
const { compile } = require("tailwindcss");
const { parse } = require("postcss");

let fixture;
before(async () => {
  fixture = await componentsDom(String.raw`
import { createElement as h } from "react";
import { render, cleanup, fireEvent } from "@testing-library/react";
import * as ds from DS_ENTRY;
import { text } from LIB_ENTRY;

const title = text("app.title", "ko");
const desc = text("ds.description", "ko");
const retry = text("ds.retry", "ko");

window.dsExports = Object.keys(ds).sort();
window.cleanup = cleanup;
window.fire = fireEvent;

window.showPage = (withAction, disabled) => {
  cleanup();
  let clicks = 0;
  window.pageClicks = () => clicks;
  const actions = withAction
    ? { label: retry, onPress: () => { clicks += 1; }, disabled }
    : undefined;
  return render(
    h(
      ds.Page,
      { title, actions },
      h(ds.Section, { title: desc }, h(ds.Text, null, desc)),
    ),
  );
};

window.showSection = () => {
  cleanup();
  return render(h(ds.Section, { title }, h(ds.Text, null, desc)));
};

window.showGrid = (span) => {
  cleanup();
  return render(
    h(
      ds.Grid,
      null,
      h(ds.GridItem, { span }, h(ds.Text, null, desc)),
    ),
  );
};

window.showStack = (gap, mode) => {
  cleanup();
  const kids =
    mode === "single"
      ? h(ds.Text, null, title)
      : mode === "keyed"
        ? [
            h(ds.Text, { key: "first" }, title),
            h(ds.Text, { key: "second" }, desc),
          ]
        : [h(ds.Text, null, title), h(ds.Text, null, desc)];
  return render(h(ds.Stack, { gap }, kids));
};

window.showInline = (props) => {
  cleanup();
  return render(
    h(
      ds.Inline,
      props,
      h(ds.Text, null, title),
      h(ds.Text, null, desc),
    ),
  );
};

window.showSplit = (size) => {
  cleanup();
  return render(
    h(ds.Split, {
      size,
      children: [h(ds.Text, null, title), h(ds.Text, null, desc)],
    }),
  );
};

window.showToolbar = () => {
  cleanup();
  return render(
    h(
      ds.Toolbar,
      null,
      h(ds.Button, { size: "control-height-sm" }, retry),
      h(ds.Divider, { direction: "vertical" }),
    ),
  );
};

window.showDivider = (direction) => {
  cleanup();
  return render(h(ds.Divider, direction === undefined ? null : { direction }));
};
`);
});
after(() => fixture?.close());

test("DS11 exports the nine new layout components and keeps ScrollArea internal", () => {
  const w = fixture.open();
  const names = Array.from(w.dsExports);
  for (const component of [
    "Page",
    "Section",
    "Grid",
    "GridItem",
    "Stack",
    "Inline",
    "Split",
    "Card",
    "Toolbar",
    "Divider",
  ])
    assert.ok(names.includes(component), component);
  assert.equal(names.includes("ScrollArea"), false);
});

test("DS11 Page, Section, Toolbar, and Divider render semantic structure without nested scroll", () => {
  const w = fixture.open();
  w.showPage(true, "enabled");
  assert.equal(w.document.querySelector("h1")?.textContent, "주식 코파일럿");
  assert.equal(
    w.document.querySelector("h2")?.textContent,
    "설명을 확인해 주세요",
  );
  assert.equal(w.document.querySelector("[data-scroll-viewport]"), null);
  assert.ok(w.document.querySelector(".\\@container"));
  assert.ok(w.document.querySelector(".max-w-content-max"));
  assert.ok(w.document.querySelector(".p-space-3"));
  assert.ok(w.document.querySelector(".\\@content-medium\\:p-space-6"));
  assert.ok(w.document.querySelector(".gap-space-6"));
  const actionButton = w.document.querySelector("header button");
  assert.ok(actionButton);
  assert.ok(actionButton.classList.contains("min-h-control-height-sm"));
  w.fire.click(actionButton);
  assert.equal(w.pageClicks(), 1);

  w.showPage(true, "disabled");
  const disabledButton = w.document.querySelector("header button");
  assert.ok(disabledButton.disabled);
  w.fire.click(disabledButton);
  assert.equal(w.pageClicks(), 0);

  w.showPage(false);
  assert.equal(w.document.querySelector("header button"), null);

  w.showSection();
  const section = w.document.querySelector("section");
  assert.ok(section.classList.contains("gap-space-3"));
  assert.equal(section.querySelector("h2")?.textContent, "주식 코파일럿");

  w.showToolbar();
  assert.equal(w.document.querySelector('[role="toolbar"]'), null);
  const toolbar = w.document.body.firstElementChild.firstElementChild;
  assert.ok(toolbar.classList.contains("min-h-toolbar-height"));
  assert.ok(toolbar.classList.contains("flex-wrap"));

  w.showDivider();
  const horizontal = w.document.querySelector('[role="separator"]');
  assert.equal(horizontal.getAttribute("aria-orientation"), "horizontal");
  assert.ok(horizontal.classList.contains("h-border-hairline"));
  assert.ok(horizontal.classList.contains("bg-line"));

  w.showDivider("vertical");
  const vertical = w.document.querySelector('[role="separator"]');
  assert.equal(vertical.getAttribute("aria-orientation"), "vertical");
  assert.ok(vertical.classList.contains("w-border-hairline"));
  assert.ok(vertical.classList.contains("self-stretch"));
  assert.ok(vertical.classList.contains("min-h-control-height-sm"));
});

test("DS11 Grid, GridItem, Stack, Inline, and Split apply classes and child min-width wrappers", () => {
  const w = fixture.open();
  for (const span of [3, 4, 6, 8, 9, 12]) {
    w.showGrid(span);
    assert.ok(w.document.querySelector(".\\@container"));
    const grid = w.document.querySelector(".grid.grid-cols-layout.gap-space-6");
    assert.ok(grid);
    const item = grid.firstElementChild;
    assert.ok(item.classList.contains(`col-span-${span}`), String(span));
    assert.ok(item.classList.contains("scroll-flex"), String(span));
  }

  w.showGrid({ narrow: 12, medium: 6, wide: 4 });
  const responsive =
    w.document.querySelector(".grid-cols-layout").firstElementChild;
  assert.ok(responsive.classList.contains("col-span-12"));
  assert.ok(responsive.classList.contains("@content-medium:col-span-6"));
  assert.ok(responsive.classList.contains("@content-wide:col-span-4"));

  w.showGrid({ medium: 6 });
  const cascaded =
    w.document.querySelector(".grid-cols-layout").firstElementChild;
  assert.ok(cascaded.classList.contains("col-span-12"));
  assert.ok(cascaded.classList.contains("@content-medium:col-span-6"));
  assert.equal(
    [...cascaded.classList].some((c) => c.startsWith("@content-wide:")),
    false,
  );

  for (const [gap, expectedGap] of [
    [undefined, "gap-space-4"],
    ["space-1", "gap-space-1"],
    ["space-12", "gap-space-12"],
  ]) {
    for (const mode of ["single", "unkeyed", "keyed"]) {
      w.showStack(gap, mode);
      const stack = w.document.body.firstElementChild.firstElementChild;
      assert.ok(stack.classList.contains("flex-col"));
      assert.ok(stack.classList.contains(expectedGap));
      assert.equal(stack.children.length, mode === "single" ? 1 : 2);
      for (const child of stack.children)
        assert.ok(child.classList.contains("scroll-flex"));
    }
  }

  w.showInline({});
  const defaultInline = w.document.body.firstElementChild.firstElementChild;
  assert.ok(defaultInline.classList.contains("gap-space-2"));
  assert.ok(defaultInline.classList.contains("items-center"));
  assert.ok(defaultInline.classList.contains("justify-start"));
  assert.equal(defaultInline.classList.contains("flex-wrap"), false);
  assert.equal(defaultInline.children.length, 2);
  for (const child of defaultInline.children)
    assert.ok(child.classList.contains("scroll-flex"));

  w.showInline({
    gap: "space-8",
    align: "end",
    justify: "between",
    wrap: "wrap",
  });
  const wrappedInline = w.document.body.firstElementChild.firstElementChild;
  assert.ok(wrappedInline.classList.contains("gap-space-8"));
  assert.ok(wrappedInline.classList.contains("items-end"));
  assert.ok(wrappedInline.classList.contains("justify-between"));
  assert.ok(wrappedInline.classList.contains("flex-wrap"));

  for (const size of [
    "sidebar-width",
    "sidebar-collapsed",
    "chat-width",
    "chat-width-min",
  ]) {
    w.showSplit(size);
    const split = w.document.body.firstElementChild.firstElementChild;
    assert.ok(split.classList.contains("flex-wrap"));
    assert.ok(split.classList.contains("gap-space-6"));
    assert.ok(split.children[0].classList.contains(`w-${size}`), size);
    assert.ok(split.children[0].classList.contains("scroll-flex"), size);
    assert.ok(split.children[1].classList.contains("flex-1"), size);
    assert.ok(split.children[1].classList.contains("wrap-slot"), size);
    assert.ok(
      split.children[1].firstElementChild.classList.contains("scroll-flex"),
    );
  }
});

test("DS11 Tailwind 4.3.5 compiles container-query spans in cascading order and ESLint rejects foreign spans", async () => {
  const tokensCss = readFileSync(
    join(desktopRoot, "src/shared/ds/tokens.css"),
    "utf8",
  );
  const themeCss = readFileSync(
    join(desktopRoot, "src/shared/ds/theme.css"),
    "utf8",
  ).replace(/^@import[^;]*;$/gmu, "");
  const source = `${readFileSync(join(desktopRoot, "node_modules/tailwindcss/theme.css"), "utf8")}\n${tokensCss}\n${themeCss}\n@tailwind utilities;`;
  const compiled = (await compile(source)).build([
    "@container",
    "grid-cols-layout",
    "self-stretch",
    "col-span-3",
    "col-span-4",
    "col-span-6",
    "col-span-8",
    "col-span-9",
    "col-span-12",
    "@content-medium:col-span-6",
    "@content-wide:col-span-4",
  ]);
  const ast = parse(compiled);
  const contexts = [];
  ast.walkRules((rule) => {
    const parent =
      rule.parent?.type === "atrule"
        ? `${rule.parent.name} ${rule.parent.params}`
        : "";
    if (rule.selector.includes("col-span-"))
      contexts.push({ selector: rule.selector, parent });
  });
  const baseIdx = contexts.findIndex((c) => c.selector === ".col-span-12");
  const mediumIdx = contexts.findIndex((c) =>
    c.selector.includes("@content-medium"),
  );
  const wideIdx = contexts.findIndex((c) =>
    c.selector.includes("@content-wide"),
  );
  assert.ok(baseIdx >= 0 && mediumIdx > baseIdx && wideIdx > mediumIdx);
  assert.match(contexts[mediumIdx].parent, /56\.25rem/u);
  assert.match(contexts[wideIdx].parent, /75rem/u);
  assert.match(compiled, /align-self:\s*stretch/u);

  const engine = new ESLint({
    cwd: desktopRoot,
    overrideConfigFile: true,
    overrideConfig: dsConfigs(),
  });
  for (const allowed of [
    ["src/shared/ds/GridItem.tsx", "col-span-3"],
    ["src/shared/ds/GridItem.tsx", "col-span-4"],
    ["src/shared/ds/GridItem.tsx", "col-span-6"],
    ["src/shared/ds/GridItem.tsx", "col-span-8"],
    ["src/shared/ds/GridItem.tsx", "col-span-9"],
    ["src/shared/ds/GridItem.tsx", "col-span-12"],
    ["src/shared/ds/GridItem.tsx", "@content-medium:col-span-6"],
    ["src/shared/ds/GridItem.tsx", "@content-wide:col-span-4"],
    ["src/shared/ds/Divider.tsx", "self-stretch"],
  ]) {
    const [result] = await engine.lintText(
      `const probe = <div className="${allowed[1]}" />;`,
      { filePath: allowed[0] },
    );
    assert.deepEqual(result.messages, [], allowed.join(":"));
  }
  for (const [file, cls] of [
    ["src/shared/ds/GridItem.tsx", "col-span-1"],
    ["src/shared/ds/GridItem.tsx", "col-span-2"],
    ["src/shared/ds/GridItem.tsx", "col-span-5"],
    ["src/shared/ds/GridItem.tsx", "col-span-7"],
    ["src/shared/ds/GridItem.tsx", "col-span-10"],
    ["src/shared/ds/GridItem.tsx", "col-span-11"],
    ["src/shared/ds/GridItem.tsx", "col-span-full"],
    ["src/shared/ds/GridItem.tsx", "col-span-[5]"],
    ["src/shared/ds/GridItem.tsx", "@content-medium:col-span-5"],
    ["src/shared/ds/GridItem.tsx", "@content-wide:col-span-7"],
    ["src/shared/ds/Inline.tsx", "items-baseline"],
    ["src/shared/ds/Inline.tsx", "items-stretch"],
    ["src/shared/ds/Split.tsx", "w-dialog-width"],
    ["src/shared/ds/Split.tsx", "w-prose-max"],
    ["src/shared/ds/Split.tsx", "w-content-max"],
    ["src/shared/ds/Stack.tsx", "max-w-content-max"],
    ["src/shared/ds/Stack.tsx", "w-sidebar-width"],
    ["src/shared/ds/Stack.tsx", "h-border-hairline"],
    ["src/shared/ds/Stack.tsx", "wrap-slot"],
  ]) {
    const [result] = await engine.lintText(
      `const probe = <div className="${cls}" />;`,
      { filePath: file },
    );
    assert.ok(
      result.messages.some(
        (m) => m.ruleId === "better-tailwindcss/no-restricted-classes",
      ),
      `${file}: ${cls}`,
    );
  }
});
