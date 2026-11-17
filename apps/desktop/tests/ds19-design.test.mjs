import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import { componentsDom } from "./ds-components-dom.mjs";
import { desktopRoot, repoRoot } from "./paths.mjs";
import { designUtilities } from "../eslint/ds-design-usage.mjs";
const require = createRequire(join(desktopRoot, "package.json"));
const { compile } = require("tailwindcss");
const { parse } = require("postcss");
test("DS19 EmptyState description and Skeleton lines preserve their design dimensions", async () => {
  const fixture = await componentsDom(String.raw`
import {createElement as h} from "react";
import {render,cleanup} from "@testing-library/react";
import {EmptyState,Skeleton,Dialog} from DS_ENTRY;
window.cleanup=cleanup;
window.showDialog=(wide)=>{cleanup();render(h(Dialog,{open:"open",wide,title:"기준 제목"},h("p",null,"기준 내용")));};
window.showEmpty=(description)=>{cleanup();render(h(EmptyState,{title:"기준 제목",children:description}));};
window.showSkeleton=(props)=>{cleanup();render(h(Skeleton,props));};
 `);
  try {
    const w = fixture.open();
    const description =
      "긴 설명 문구가 본문 전체 폭으로 퍼지지 않아야 합니다. ".repeat(30);
    w.showEmpty(description);
    const paragraph = w.document.querySelectorAll("p")[1];
    assert.equal(paragraph.textContent, description);
    assert.deepEqual([...paragraph.classList].sort(), [
      "max-w-dialog-width",
      "text-body-sm",
    ]);
    for (const wide of ["normal", "wide"]) {
      w.showDialog(wide);
      const popup = w.document.querySelector('[aria-modal="true"]');
      assert.ok(popup);
      assert.ok(
        popup.classList.contains(
          wide === "normal" ? "max-w-dialog-width" : "max-w-dialog-width-wide",
        ),
      );
    }
    for (const lines of [3, 6]) {
      w.showSkeleton({ lines });
      const items = w.document.querySelectorAll(".skeleton-line");
      assert.equal(items.length, lines);
      for (const item of items)
        assert.deepEqual([...item.classList].sort(), [
          "bg-surface-sunken",
          "rounded-sm",
          "skeleton-line",
        ]);
    }
    for (const height of [
      "chart-height-sm",
      "chart-height",
      "chart-height-lg",
    ]) {
      w.showSkeleton({ variant: "block", height });
      const block = w.document.querySelector('[aria-hidden="true"]');
      assert.equal(block.childElementCount, 0);
      assert.ok(block.classList.contains(`h-${height}`));
    }
  } finally {
    fixture.close();
  }
  const tokens = JSON.parse(
    readFileSync(join(desktopRoot, "design-tokens/tokens.json"), "utf8"),
  );
  const tokenCss = readFileSync(
    join(desktopRoot, "src/shared/ds/tokens.css"),
    "utf8",
  );
  const source = [
    readFileSync(
      join(desktopRoot, "node_modules/tailwindcss/theme.css"),
      "utf8",
    ),
    tokenCss,
    ...designUtilities(tokens).map(
      ([name, property, value]) => `@utility ${name}{${property}:${value};}`,
    ),
    "@tailwind utilities;",
  ].join("\n");
  const css = parse(
    (await compile(source)).build(["max-w-dialog-width", "skeleton-line"]),
  );
  const declarations = {};
  css.walkRules((rule) => {
    if ([".max-w-dialog-width", ".skeleton-line"].includes(rule.selector))
      rule.walkDecls((decl) => {
        declarations[`${rule.selector}:${decl.prop}`] = decl.value;
      });
  });
  assert.equal(
    declarations[".max-w-dialog-width:max-width"],
    "var(--spacing-dialog-width)",
  );
  assert.equal(declarations[".skeleton-line:height"], "var(--text-size-sm)");
  const original = JSON.parse(
    readFileSync(join(repoRoot, "design/tokens.json"), "utf8"),
  );
  const allTokens = Object.values(original).flatMap(
    (group) => group.tokens ?? [],
  );
  const pixels = (value) =>
    value.endsWith("rem") ? parseFloat(value) * 16 : parseFloat(value);
  const compiledValues = {};
  parse(tokenCss).walkDecls((decl) => {
    compiledValues[decl.prop] = decl.value;
  });
  assert.equal(
    pixels(compiledValues["--spacing-dialog-width"]),
    pixels(allTokens.find((token) => token.name === "dialog-width").value),
  );
  assert.equal(
    pixels(compiledValues["--text-size-sm"]),
    pixels(allTokens.find((token) => token.name === "text-sm").value),
  );
});
