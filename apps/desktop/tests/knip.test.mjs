import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  APP_FLAGS,
  PRODUCTION_FLAGS,
  productionCaseVerdict,
  productionConfig,
  readProductionAllowlist,
  caseProblems,
  caseVerdict,
  desktop,
  readAppConfig,
  readCases,
  runKnip,
  withConfig,
} from "../../../harness/knip-check/knip.mjs";
import { repoRoot } from "./paths.mjs";

const appConfig = readAppConfig();
const { cases, addedWithDependencies, production } = readCases();

const casesExpecting = (type) =>
  Object.entries(cases)
    .filter(([, types]) => types.includes(type))
    .map(([name]) => name);

const caseSurvives = (config, names) =>
  withConfig(config, (file) =>
    names.every(
      (name) => caseProblems(cases[name], caseVerdict(name, file)).length === 0,
    ),
  );

const appPasses = (config) =>
  withConfig(config, (file) => {
    const run = runKnip(desktop, file, APP_FLAGS);
    return run.status === 0 && run.types.length === 0;
  });

test("knip is pinned to the version of harness 4.3", () => {
  const manifest = JSON.parse(
    readFileSync(join(desktop, "package.json"), "utf8"),
  );
  assert.equal(manifest.devDependencies.knip, "6.45.1");
});

test("turning off or excluding any issue type makes its cases fail", () => {
  const survivors = Object.keys(appConfig.rules)
    .filter((type) => casesExpecting(type).length > 0)
    .flatMap((type) => [
      {
        label: `${type} off`,
        config: { ...appConfig, rules: { ...appConfig.rules, [type]: "off" } },
        names: casesExpecting(type),
      },
      {
        label: `${type} warn`,
        config: { ...appConfig, rules: { ...appConfig.rules, [type]: "warn" } },
        names: casesExpecting(type),
      },
      ...(addedWithDependencies.includes(type)
        ? []
        : [
            {
              label: `${type} not included`,
              config: {
                ...appConfig,
                include: appConfig.include.filter((other) => other !== type),
              },
              names: casesExpecting(type),
            },
          ]),
    ])
    .filter(({ config, names }) => caseSurvives(config, names))
    .map(({ label }) => label);
  assert.deepEqual(survivors, []);
});

test("removing any entry pattern or ignore entry makes the app check fail", () => {
  const without = (key, value) => ({
    ...appConfig,
    [key]: appConfig[key].filter((other) => other !== value),
  });
  const survivors = [
    ...appConfig.entry.map((value) => ["entry", value]),
    ...appConfig.ignoreDependencies.map((value) => [
      "ignoreDependencies",
      value,
    ]),
    ...appConfig.ignoreBinaries.map((value) => ["ignoreBinaries", value]),
  ]
    .filter(([key, value]) => appPasses(without(key, value)))
    .map(([key, value]) => `${key} ${value}`);
  assert.deepEqual(survivors, []);
});

test("just check runs the knip and Vitest checks", () => {
  const justfile = readFileSync(join(repoRoot, "justfile"), "utf8");
  assert.match(justfile, /^ {4}node harness\/knip-check\/check\.mjs$/m);
  assert.match(justfile, /^ {4}node harness\/vitest-check\/check\.mjs$/m);
});

const expectedProductionAllowlist = [
  "exports src/shared/ds/AreaChart.tsx AreaChart",
  "exports src/shared/ds/ChatInput.tsx ChatInput",
  "exports src/shared/ds/ChatInput.tsx chatInputStressStories",
  "exports src/shared/ds/ChatInput.tsx chatInputStressWithDraft",
  "exports src/shared/ds/Checkbox.tsx Checkbox",
  "exports src/shared/ds/DonutChart.tsx DonutChart",
  "exports src/shared/ds/DonutChart.tsx donutArcs",
  "exports src/shared/ds/LineChart.tsx LineChart",
  "exports src/shared/ds/LineChart.tsx lineCoordinates",
  "exports src/shared/ds/Markdown.tsx Markdown",
  "exports src/shared/ds/Numbers.tsx DirectionBadge",
  "exports src/shared/ds/Numbers.tsx Probability",
  "exports src/shared/ds/SegmentedControl.tsx SegmentedControl",
  "exports src/shared/ds/Split.tsx Split",
  "exports src/shared/ds/Toolbar.tsx Toolbar",
  "exports src/shared/ds/Treemap.tsx Treemap",
  "exports src/shared/ds/Treemap.tsx treemapRectangles",
  "exports src/shared/ds/index.ts AreaChart",
  "exports src/shared/ds/index.ts ChatInput",
  "exports src/shared/ds/index.ts Checkbox",
  "exports src/shared/ds/index.ts DirectionBadge",
  "exports src/shared/ds/index.ts Divider",
  "exports src/shared/ds/index.ts DonutChart",
  "exports src/shared/ds/index.ts Heading",
  "exports src/shared/ds/index.ts Icon",
  "exports src/shared/ds/index.ts IconButton",
  "exports src/shared/ds/index.ts LineChart",
  "exports src/shared/ds/index.ts Markdown",
  "exports src/shared/ds/index.ts Probability",
  "exports src/shared/ds/index.ts SegmentedControl",
  "exports src/shared/ds/index.ts Split",
  "exports src/shared/ds/index.ts Toolbar",
  "exports src/shared/ds/index.ts Treemap",
  "exports src/shared/ipc/call.ts openExternal",
  "exports src/shared/ipc/index.ts openExternal",
  "exports src/shared/lib/brand.ts parseCount",
  "exports src/shared/lib/brand.ts parseTimestamp",
  "exports src/shared/lib/index.ts parseCount",
  "exports src/shared/lib/index.ts parseMoney",
  "exports src/shared/lib/index.ts parsePrice",
  "exports src/shared/lib/index.ts parseProbability",
  "exports src/shared/lib/index.ts parseQuantity",
  "exports src/shared/lib/index.ts parseRatio",
  "exports src/shared/lib/index.ts parseTimestamp",
  "exports src/shared/lib/money.ts parseMoney",
  "exports src/shared/lib/money.ts parsePrice",
  "exports src/shared/lib/money.ts parseProbability",
  "exports src/shared/lib/money.ts parseQuantity",
  "exports src/shared/lib/money.ts parseRatio",
  "files src/shared/ds/Base.stories.tsx src/shared/ds/Base.stories.tsx",
  "files src/shared/ds/Button.stories.tsx src/shared/ds/Button.stories.tsx",
  "files src/shared/ds/Catalog.stories.tsx src/shared/ds/Catalog.stories.tsx",
  "files src/shared/ds/Charts.stories.tsx src/shared/ds/Charts.stories.tsx",
  "files src/shared/ds/Checkbox.stories.tsx src/shared/ds/Checkbox.stories.tsx",
  "files src/shared/ds/Content.stories.tsx src/shared/ds/Content.stories.tsx",
  "files src/shared/ds/Disclosure.stories.tsx src/shared/ds/Disclosure.stories.tsx",
  "files src/shared/ds/Feedback.stories.tsx src/shared/ds/Feedback.stories.tsx",
  "files src/shared/ds/Heading.stories.tsx src/shared/ds/Heading.stories.tsx",
  "files src/shared/ds/Icon.stories.tsx src/shared/ds/Icon.stories.tsx",
  "files src/shared/ds/IconButton.stories.tsx src/shared/ds/IconButton.stories.tsx",
  "files src/shared/ds/Layout.stories.tsx src/shared/ds/Layout.stories.tsx",
  "files src/shared/ds/Numbers.stories.tsx src/shared/ds/Numbers.stories.tsx",
  "files src/shared/ds/SegmentedControl.stories.tsx src/shared/ds/SegmentedControl.stories.tsx",
  "files src/shared/ds/Select.stories.tsx src/shared/ds/Select.stories.tsx",
  "files src/shared/ds/Status.stories.tsx src/shared/ds/Status.stories.tsx",
  "files src/shared/ds/Switch.stories.tsx src/shared/ds/Switch.stories.tsx",
  "files src/shared/ds/Text.stories.tsx src/shared/ds/Text.stories.tsx",
  "files src/shared/ds/TextField.stories.tsx src/shared/ds/TextField.stories.tsx",
  "files src/shared/ds/stories.ts src/shared/ds/stories.ts",
  "files src/shared/ds/story-data.ts src/shared/ds/story-data.ts",
  "files src/shared/ds/testing/Stress.stories.tsx src/shared/ds/testing/Stress.stories.tsx",
  "files src/shared/ds/testing/StressContent.stories.tsx src/shared/ds/testing/StressContent.stories.tsx",
  "files src/shared/ds/testing/StressLayout.stories.tsx src/shared/ds/testing/StressLayout.stories.tsx",
  "files src/shared/ds/testing/messages.ts src/shared/ds/testing/messages.ts",
  "files src/shared/ds/testing/overflow-geometry.ts src/shared/ds/testing/overflow-geometry.ts",
  "files src/shared/ds/testing/overflow.ts src/shared/ds/testing/overflow.ts",
  "files src/shared/ds/testing/stress.ts src/shared/ds/testing/stress.ts",
  "files src/shared/ds/CandleChart.tsx src/shared/ds/CandleChart.tsx",
  "files src/shared/lib/canvas-chart.ts src/shared/lib/canvas-chart.ts",
  "files src/shared/platform/canvasTheme.ts src/shared/platform/canvasTheme.ts",
  "files src/shared/ipc/testing/fakes.ts src/shared/ipc/testing/fakes.ts",
  "files src/shared/ipc/testing/mockBindings.ts src/shared/ipc/testing/mockBindings.ts",
  "files src/shared/ipc/testing/responses.ts src/shared/ipc/testing/responses.ts",
  "types src/shared/ds/AppShell.tsx PanelState",
  "types src/shared/ds/index.ts ChartPoint",
  "types src/shared/ds/index.ts PanelState",
  "types src/shared/ipc/index.ts OpenExternalResult",
  "types src/shared/lib/index.ts Count",
  "types src/shared/lib/index.ts DecimalParseError",
  "types src/shared/lib/index.ts Err",
  "types src/shared/lib/index.ts MessageKey",
  "types src/shared/lib/index.ts Ok",
  "types src/shared/lib/index.ts Probability",
  "types src/shared/lib/index.ts Ratio",
  "types src/shared/lib/money.ts Probability",
  "types src/shared/lib/money.ts Ratio",
];

test("the production allowlist holds exactly the documented skeleton entries, each with a reason", () => {
  const { entries } = readProductionAllowlist();
  assert.deepEqual(
    entries.map((entry) => entry.issue),
    expectedProductionAllowlist,
  );
  assert.deepEqual(
    entries.filter(
      (entry) =>
        typeof entry.reason !== "string" ||
        entry.reason.length === 0 ||
        /[*?]/.test(entry.issue),
    ),
    [],
  );
});

test("the production run drops non-production entries and dropping any production flag makes a case fail", () => {
  assert.deepEqual(productionConfig(appConfig).entry, [
    "src/shared/ds/theme.css!",
  ]);
  const survivors = withConfig(productionConfig(appConfig), (file) =>
    PRODUCTION_FLAGS.filter((flag) =>
      Object.entries(production).every(
        ([name, expected]) =>
          caseProblems(
            expected,
            productionCaseVerdict(
              name,
              file,
              PRODUCTION_FLAGS.filter((other) => other !== flag),
            ),
          ).length === 0,
      ),
    ),
  );
  assert.deepEqual(survivors, []);
});

test("a test-only export in the app source fails the production run", () => {
  const verdict = withConfig(productionConfig(appConfig), (file) =>
    productionCaseVerdict("test-only-export", file),
  );
  assert.deepEqual(verdict, { failed: true, types: ["exports"] });
});
