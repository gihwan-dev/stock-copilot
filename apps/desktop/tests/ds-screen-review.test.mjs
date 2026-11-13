import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import ts from "typescript";
import { ESLint } from "eslint";
import { dsConfigs, SCREEN_RULES, DS_RULES } from "../eslint/ds-config.mjs";
import { desktopRoot } from "./paths.mjs";

const lint = async (
  code,
  path = "src/features/probe/ui/Probe.tsx",
  rules = {},
) =>
  (
    await new ESLint({
      cwd: desktopRoot,
      overrideConfigFile: true,
      overrideConfig: [
        ...dsConfigs(),
        { files: ["src/**/*.ts", "src/**/*.tsx"], rules },
      ],
    }).lintText(code, { filePath: path })
  )[0].messages;
test("DS09 review permits business keys modes locale tags and every generated token literal", async () => {
  const values = [
    "pe-ratio pb-ratio",
    "my-portfolio z-score",
    "pe-ratio",
    "pb-ratio",
    "p-value",
    "my-portfolio",
    "z-score",
    "pt-BR",
    "grid",
    "list",
    "block",
    "flex",
    "hidden",
  ];
  const ast = ts.createSourceFile(
    "tokens.ts",
    readFileSync(`${desktopRoot}/src/shared/ds/tokens.ts`, "utf8"),
    ts.ScriptTarget.Latest,
    true,
  );
  const visit = (node) => {
    if (ts.isStringLiteral(node)) values.push(node.text);
    ts.forEachChild(node, visit);
  };
  visit(ast);
  for (const value of new Set(values))
    assert.deepEqual(
      await lint(`const value=${JSON.stringify(value)};`),
      [],
      value,
    );
  for (const value of [
    "flex items-center gap-2",
    "hover:bg-primary",
    "focus:bg-primary",
    "md:flex",
    "p-[13px]",
    "p-(--space)",
  ])
    assert.ok(
      (await lint(`const value=${JSON.stringify(value)};`)).some(
        (m) => m.ruleId === "ds/no-screen-classes",
      ),
      value,
    );
  assert.deepEqual(
    await lint(
      'const select=(mode:"grid"|"list")=><Clicker mode={mode}/>; switch(mode){case "block":break;}',
    ),
    [],
  );
});
test("DS09 review enforces readable attributes in DS and loses witnesses when disabled", async () => {
  for (const prop of [
    "aria-label",
    "aria-description",
    "aria-valuetext",
    "aria-roledescription",
    "aria-placeholder",
    "aria-braillelabel",
    "aria-brailleroledescription",
    "aria-colindextext",
    "aria-rowindextext",
    "title",
    "alt",
    "placeholder",
    "label",
  ]) {
    const code = `<button ${prop}="Close dialog"/>`;
    assert.ok(
      (await lint(code, "src/shared/ds/Probe.tsx")).some(
        (m) => m.ruleId === "ds/no-ui-attributes",
      ),
      prop,
    );
    assert.deepEqual(
      await lint(code, "src/shared/ds/Probe.tsx", {
        "ds/no-ui-attributes": "off",
      }),
      [],
    );
  }
});
test("DS09 review checks catalog values for minus and emoji while permitting copyright", async () => {
  for (const value of ["-3", "🇰🇷", "1️⃣", "😀"]) {
    const code = `const catalog={value:{ko:${JSON.stringify(value)}}};`;
    const rules = Object.fromEntries(
      [
        ...new Set([...Object.keys(SCREEN_RULES), ...Object.keys(DS_RULES)]),
      ].map((key) => [key, "off"]),
    );
    assert.ok(
      (
        await lint(code, "src/features/probe/messages.ts", {
          ...rules,
          "ds/no-message-format": "error",
        })
      ).some((m) => m.ruleId === "ds/no-message-format"),
      value,
    );
    assert.deepEqual(
      await lint(code, "src/features/probe/messages.ts", {
        ...rules,
        "ds/no-message-format": "off",
      }),
      [],
    );
  }
  assert.deepEqual(
    await lint(
      'const catalog={value:{ko:"© 2026, −3"}};',
      "src/features/probe/messages.ts",
    ),
    [],
  );
});

test("DS09 review DS cannot turn an arbitrary string prop into a class", async () => {
  const code = "const Button=({paint})=><div className={paint}/>;";
  assert.ok(
    (await lint(code, "src/shared/ds/Probe.tsx")).some(
      (m) => m.ruleId === "ds/static-classes",
    ),
  );
});

test("DS09 review checks all formatting methods and distinguishes text from ID references", async () => {
  for (const method of [
    "toLocaleString",
    "toLocaleDateString",
    "toLocaleTimeString",
    "toFixed",
    "toExponential",
    "toPrecision",
  ]) {
    assert.ok(
      (await lint(`const value=source.${method}();`)).some(
        (message) => message.ruleId === "ds/no-screen-format",
      ),
      method,
    );
  }
  assert.deepEqual(
    await lint(
      '<button aria-labelledby="caption" aria-describedby="hint"/>',
      "src/shared/ds/Probe.tsx",
    ),
    [],
  );
  for (const kind of ["button", "submit", "reset"])
    assert.ok(
      (
        await lint(
          `<input type="${kind}" value="Save"/>`,
          "src/shared/ds/Probe.tsx",
        )
      ).some((message) => message.ruleId === "ds/no-ui-attributes"),
    );
  assert.deepEqual(
    await lint(
      '<input type="text" value="content"/>',
      "src/shared/ds/Probe.tsx",
    ),
    [],
  );
});
