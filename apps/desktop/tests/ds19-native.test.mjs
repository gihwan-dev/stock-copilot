import assert from "node:assert/strict";
import test from "node:test";
import { ESLint } from "eslint";
import { dsConfigs, DS_RULES } from "../eslint/ds-config.mjs";
import { desktopRoot } from "./paths.mjs";
const rule = "ds/no-presentation-props";
const engine = new ESLint({
  cwd: desktopRoot,
  overrideConfigFile: true,
  overrideConfig: dsConfigs(
    Object.fromEntries(
      Object.keys(DS_RULES).map((key) => [key, key === rule ? "error" : "off"]),
    ),
  ),
});
test("DS19 native overlay and sizing attributes cannot bypass class restrictions", async () => {
  for (const code of [
    '<div popover="manual"/>',
    "<dialog open/>",
    '<pre className="text-body"/>',
    "<textarea/>",
    '<textarea wrap="off"/>',
    "<textarea cols={200}/>",
    "<textarea rows={50}/>",
  ]) {
    const [result] = await engine.lintText(`const probe=${code};`, {
      filePath: "src/shared/ds/Badge.tsx",
    });
    assert.ok(
      result.messages.some((m) => m.ruleId === rule),
      code,
    );
  }
  for (const code of [
    "<textarea/>",
    '<textarea wrap="soft"/>',
    '<textarea wrap="hard"/>',
  ]) {
    const [result] = await engine.lintText(`const probe=${code};`, {
      filePath: "src/shared/ds/ChatInput.tsx",
    });
    assert.deepEqual(result.messages, [], code);
  }
  for (const code of [
    '<textarea wrap="off"/>',
    "<textarea cols={200}/>",
    "<textarea rows={50}/>",
  ]) {
    const [result] = await engine.lintText(`const probe=${code};`, {
      filePath: "src/shared/ds/ChatInput.tsx",
    });
    assert.ok(
      result.messages.some((m) => m.ruleId === rule),
      code,
    );
  }
  for (const code of ['<pre className="chat-content font-mono"/>', "<div/>"]) {
    const [result] = await engine.lintText(`const probe=${code};`, {
      filePath: "src/shared/ds/Markdown.tsx",
    });
    assert.deepEqual(result.messages, [], code);
  }
});

test("DS19 generated semantic utilities carry every risky declaration into file policy", async () => {
  const { utilityDeclarations, declarationPolicies } =
    await import("../eslint/ds-overflow-utilities.mjs");
  const { overflowRestrictions } = await import("../eslint/ds-overflow.mjs");
  const { createRequire } = await import("node:module");
  const { join } = await import("node:path");
  const require = createRequire(join(desktopRoot, "package.json"));
  const { compile } = require("tailwindcss");
  const { parse } = require("postcss");
  const examples = [
    ["overflow-y", "auto", [18, 19]],
    ["position", "absolute", [20]],
    ["max-width", "20rem", [21]],
    ["height", "100dvh", [21, 22]],
    ["white-space", "nowrap", [23]],
    ["margin-left", "-1rem", [24]],
    ["transform", "translateX(100%)", [24]],
    ["overflow-wrap", "anywhere", []],
    ["white-space", "pre-wrap", []],
    ["width", "100%", []],
    ["height", "auto", []],
    ["min-width", "0", []],
  ];
  const original = utilityDeclarations.length;
  try {
    for (const [index, [property, value, wanted]] of examples.entries()) {
      const name = `future-semantic-${index}`;
      const compiled = (
        await compile(
          `@utility ${name}{${property}:${value}} @tailwind utilities;`,
        )
      ).build([name]);
      const actual = [];
      parse(compiled).walkRules((rule) => {
        if (rule.selector === `.${name}`)
          rule.walkDecls((decl) => actual.push([decl.prop, decl.value]));
      });
      assert.deepEqual(actual, [[property, value]]);
      assert.deepEqual(declarationPolicies(property, value), wanted);
      utilityDeclarations.push([name, property, value]);
      const found = overflowRestrictions("")
        .filter((restriction) =>
          new RegExp(restriction.pattern, "u").test(name),
        )
        .map((restriction) => Number(/#(\d+):/u.exec(restriction.message)[1]));
      assert.deepEqual(found, wanted, name);
    }
  } finally {
    utilityDeclarations.splice(original);
  }
});
