import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { ESLint } from "eslint";
import { dsConfigs, DS_RULES } from "../eslint/ds-config.mjs";
import { cssBoundaryProblems } from "../../../harness/ds-style/check.mjs";
import { desktopRoot } from "./paths.mjs";
const witnesses = JSON.parse(
  readFileSync(
    new URL(
      "../../../harness/eslint-violations/ds19-overflow.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
const restricted = "better-tailwindcss/no-restricted-classes";
const rules = Object.fromEntries(
  Object.entries(DS_RULES).map(([key, value]) => [
    key,
    key === restricted ? value : "off",
  ]),
);
const engine = new ESLint({
  cwd: desktopRoot,
  overrideConfigFile: true,
  overrideConfig: dsConfigs(rules),
});
test("DS19 registered violations and exact file controls enforce all eight policies", async () => {
  assert.deepEqual(
    [...new Set(witnesses.map((w) => w.row))].sort(),
    [18, 19, 20, 21, 22, 23, 24, 25],
  );
  for (const witness of witnesses) {
    const [result] = await engine.lintText(witness.code, {
      filePath: witness.file,
    });
    const found = result.messages
      .filter(
        (m) => m.ruleId === restricted || m.ruleId === "no-restricted-imports",
      )
      .map((m) => m.ruleId);
    assert.equal(
      found.includes(witness.rule),
      witness.rule !== null,
      `${witness.name}: ${JSON.stringify(result.messages)}`,
    );
    if (witness.rule === null) assert.deepEqual(found, [], witness.name);
  }
});
test("DS19 existing CSS gate rejects each overflow declaration and viewport unit", () => {
  for (const declaration of [
    "overflow:hidden",
    "overflow-x:auto",
    "overflow-y:scroll",
    "position:absolute",
    "position:fixed",
    "position:sticky",
    "inset:0",
    "height:var(--spacing-control-height)",
    "width:var(--spacing-dialog-width)",
    "max-width:var(--spacing-prose-max)",
    "height:100vh",
    "height:100dvh",
    "height:100svh",
    "height:100lvh",
    "width:100vw",
    "width:100vmin",
    "width:100vmax",
    "white-space:nowrap",
    "margin:-1rem",
    "translate:100%",
    "transform:scale(2)",
  ]) {
    assert.deepEqual(
      cssBoundaryProblems("extra.css", `.probe{${declaration}}`),
      ["handwritten CSS forbidden"],
    );
    assert.deepEqual(
      cssBoundaryProblems("theme.css", `.probe{${declaration}}`),
      ["theme entry changed"],
    );
  }
  assert.deepEqual(
    cssBoundaryProblems(
      "theme.css",
      readFileSync(
        new URL("../src/shared/ds/theme.css", import.meta.url),
        "utf8",
      ),
    ),
    [],
  );
});

test("DS19 each numbered restriction has an independent witness without the general token ban", async () => {
  const { overflowRestrictions } = await import("../eslint/ds-overflow.mjs");
  for (const witness of witnesses.filter(
    (w) => w.row !== 25 && w.name !== "Text-no-line-clamp",
  )) {
    const isolated = new ESLint({
      cwd: desktopRoot,
      overrideConfigFile: true,
      overrideConfig: [
        ...dsConfigs(
          Object.fromEntries(Object.keys(DS_RULES).map((key) => [key, "off"])),
        ),
        {
          files: ["src/shared/ds/**/*.tsx"],
          rules: {
            [restricted]: [
              "error",
              {
                restrict: overflowRestrictions(witness.file).filter((r) =>
                  r.message.includes(`#${witness.row}:`),
                ),
              },
            ],
          },
        },
      ],
    });
    const [result] = await isolated.lintText(witness.code, {
      filePath: witness.file,
    });
    assert.equal(
      result.messages.some((m) => m.ruleId === restricted),
      witness.rule !== null,
      witness.name,
    );
  }
});

test("DS19 every documented file exception is exact and does not extend to a sibling folder", async () => {
  const groups = [
    ["truncate", ["ScrollArea", "Text", "Heading"]],
    ["overflow-auto", ["ScrollArea"]],
    [
      "fixed",
      ["Dialog", "Popover", "Menu", "Tooltip", "Toast", "AppShell", "Table"],
    ],
    [
      "h-control-height",
      [
        "Button",
        "button-variants",
        "IconButton",
        "TextField",
        "Select",
        "Combobox",
        "Switch",
        "Checkbox",
        "SegmentedControl",
        "Table",
        "ChartFrame",
        "Toolbar",
        "StatusBar",
        "AppShell",
        "Dialog",
        "ChatInput",
        "Icon",
        "Disclosure",
        "Card",
      ],
    ],
    ["h-screen", ["AppShell"]],
    [
      "whitespace-nowrap",
      ["Numbers", "Table", "Badge", "Button", "button-variants", "IconButton"],
    ],
  ];
  for (const [value, files] of groups) {
    for (const name of files) {
      const code = `const probe=<div className="${value}"/>;`;
      const [allowed] = await engine.lintText(code, {
        filePath: `src/shared/ds/${name}.tsx`,
      });
      assert.deepEqual(allowed.messages, [], `${name}: ${value}`);
      const [nested] = await engine.lintText(code, {
        filePath: `src/shared/ds/nested/${name}.tsx`,
      });
      assert.ok(
        nested.messages.some((m) => m.ruleId === restricted),
        `${name} nested: ${value}`,
      );
    }
  }
  const { OFFSET_FILES } = await import("../eslint/ds-overflow.mjs");
  assert.deepEqual(OFFSET_FILES, []);
});

test("DS19 ChartTable keeps only its pre-existing sr-only positioning exception", async () => {
  const actual = new ESLint({
    cwd: desktopRoot,
    overrideConfigFile: true,
    overrideConfig: dsConfigs(),
  });
  const [control] = await actual.lintText(
    'const probe=<table className="sr-only"/>;',
    { filePath: "src/shared/ds/ChartTable.tsx" },
  );
  assert.deepEqual(control.messages, []);
  for (const value of ["absolute", "fixed", "sticky", "hover:fixed"]) {
    const [result] = await actual.lintText(
      `const probe=<div className="${value}"/>;`,
      { filePath: "src/shared/ds/ChartTable.tsx" },
    );
    assert.ok(
      result.messages.some((message) => message.ruleId === restricted),
      value,
    );
  }
});

test("DS19 policy files equal the documented set without adding a new helper", async () => {
  const { OVERFLOW_POLICIES } = await import("../eslint/ds-overflow.mjs");
  const ds = (names) => names.map((name) => `src/shared/ds/${name}.tsx`);
  const expected = [
    [18, ds(["ScrollArea", "Text", "Heading"])],
    [19, ds(["ScrollArea"])],
    [
      20,
      ds([
        "Dialog",
        "Popover",
        "Menu",
        "Tooltip",
        "Toast",
        "AppShell",
        "Table",
      ]),
    ],
    [
      21,
      ds([
        "Button",
        "button-variants",
        "IconButton",
        "TextField",
        "Select",
        "Combobox",
        "Switch",
        "Checkbox",
        "SegmentedControl",
        "Table",
        "ChartFrame",
        "Toolbar",
        "StatusBar",
        "AppShell",
        "Dialog",
        "ChatInput",
        "Icon",
        "Disclosure",
        "Card",
      ]),
    ],
    [22, ds(["AppShell"])],
    [
      23,
      ds([
        "Numbers",
        "Table",
        "Badge",
        "Button",
        "button-variants",
        "IconButton",
      ]),
    ],
    [24, []],
  ];
  assert.deepEqual(
    OVERFLOW_POLICIES.map(([id, files]) => [id, files]),
    expected,
  );
});

test("DS19 position policy set rejects an added helper independently of length decisions", async () => {
  const { POSITION_FILES } = await import("../eslint/ds-overflow.mjs");
  assert.deepEqual(
    POSITION_FILES,
    ["Dialog", "Popover", "Menu", "Tooltip", "Toast", "AppShell", "Table"].map(
      (name) => `src/shared/ds/${name}.tsx`,
    ),
  );
});

test("DS19 component dimension exceptions equal the approved exact file token pairs", async () => {
  const { LENGTH_EXCEPTIONS } = await import("../eslint/ds-overflow.mjs");
  assert.deepEqual(LENGTH_EXCEPTIONS, [
    ["src/shared/ds/Markdown.tsx", ["markdown-content", "max-w-prose-max"]],
    ["src/shared/ds/EmptyState.tsx", ["max-w-dialog-width"]],
    [
      "src/shared/ds/Skeleton.tsx",
      [
        "skeleton-line",
        "h-chart-height-sm",
        "h-chart-height",
        "h-chart-height-lg",
      ],
    ],
  ]);
  for (const [file, values] of LENGTH_EXCEPTIONS) {
    for (const value of [...values, ...values.map((v) => `hover:${v}`)]) {
      const [result] = await engine.lintText(
        `const probe=<div className="${value}"/>;`,
        { filePath: file },
      );
      assert.deepEqual(result.messages, [], `${file}: ${value}`);
    }
    for (const value of [
      "h-control-height",
      "w-dialog-width",
      "max-w-dialog-width-wide",
      "h-[37px]",
      "max-w-[41rem]",
      ...(file.includes("Skeleton")
        ? ["max-w-dialog-width"]
        : ["skeleton-line", "h-chart-height"]),
    ]) {
      const [result] = await engine.lintText(
        `const probe=<div className="${value}"/>;`,
        { filePath: file },
      );
      assert.ok(
        result.messages.some((m) => m.ruleId === restricted),
        `${file}: ${value}`,
      );
    }
  }
});
