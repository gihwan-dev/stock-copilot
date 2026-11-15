import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import { desktopRoot } from "./paths.mjs";
import { designUtilities } from "../eslint/ds-design-usage.mjs";
const require = createRequire(join(desktopRoot, "package.json"));
const { compile } = require("tailwindcss");
const postcss = require("postcss");
test("DS18 generated CSS restores readable markdown after preflight", async () => {
  const read = (file) => readFileSync(join(desktopRoot, file), "utf8");
  const tokens = JSON.parse(read("design-tokens/tokens.json"));
  const source = [
    read("node_modules/tailwindcss/theme.css"),
    read("node_modules/tailwindcss/preflight.css"),
    read("src/shared/ds/tokens.css"),
    ...designUtilities(tokens).map(
      ([name, property, value]) =>
        `@utility ${name} { ${property}: ${value}; }`,
    ),
    "@tailwind utilities;",
  ].join("\n");
  const css = (await compile(source)).build([
    "markdown-ordered",
    "markdown-bullets",
    "markdown-quote",
    "text-title-lg",
    "text-title-md",
    "text-title-sm",
    "my-space-3",
    "font-mono",
  ]);
  const ast = postcss.parse(css);
  const declarations = (selector) => {
    const result = {};
    ast.walkRules((rule) => {
      if (rule.selector === selector)
        rule.walkDecls((decl) => {
          result[decl.prop] = decl.value;
        });
    });
    return result;
  };
  assert.match(css, /list-style:\s*none/u);
  assert.equal(declarations(".markdown-ordered")["list-style-type"], "decimal");
  assert.equal(declarations(".markdown-bullets")["list-style-type"], "disc");
  for (const selector of [".markdown-ordered", ".markdown-bullets"])
    assert.equal(
      declarations(selector)["padding-inline-start"],
      "var(--spacing-space-6)",
    );
  for (const size of ["lg", "md", "sm"]) {
    const heading = declarations(`.text-title-${size}`);
    assert.ok(heading["font-size"] && heading["font-size"] !== "inherit");
    assert.ok(heading["font-weight"] && heading["font-weight"] !== "inherit");
  }
  assert.notEqual(
    declarations(".text-title-lg")["font-size"],
    declarations(".text-title-sm")["font-size"],
  );
  assert.match(declarations(".my-space-3")["margin-block"], /space-3/u);
  assert.match(
    declarations(".markdown-quote")["border-inline-start-color"],
    /line-strong/u,
  );
  assert.equal(
    declarations(".markdown-quote")["border-inline-start-width"],
    "var(--border-width-hairline)",
  );
  assert.equal(
    declarations(".markdown-quote")["border-inline-start-style"],
    "solid",
  );
  assert.ok(declarations(".font-mono")["font-family"]);
});
