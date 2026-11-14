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
  "files src/shared/ds/index.ts src/shared/ds/index.ts",
  "files src/shared/ipc/index.ts src/shared/ipc/index.ts",
  "dependencies package.json @tauri-apps/api",
  "files src/shared/ipc/bindings.ts src/shared/ipc/bindings.ts",
  "files src/shared/ipc/call.ts src/shared/ipc/call.ts",
  "files src/shared/ipc/schema.ts src/shared/ipc/schema.ts",
  "files src/shared/ipc/testing/fakes.ts src/shared/ipc/testing/fakes.ts",
  "files src/shared/ipc/testing/mockBindings.ts src/shared/ipc/testing/mockBindings.ts",
  "files src/shared/ipc/testing/responses.ts src/shared/ipc/testing/responses.ts",
  "dependencies package.json @base-ui/react",
  "exports src/shared/lib/brand.ts emptyInputValue",
  "exports src/shared/lib/brand.ts parseCount",
  "exports src/shared/lib/brand.ts parseEntityId",
  "exports src/shared/lib/brand.ts parseTicker",
  "exports src/shared/lib/brand.ts parseTimestamp",
  "exports src/shared/lib/index.ts emptyInputValue",
  "exports src/shared/lib/index.ts ok",
  "exports src/shared/lib/index.ts parseCount",
  "exports src/shared/lib/index.ts parseEntityId",
  "exports src/shared/lib/index.ts parseMoney",
  "exports src/shared/lib/index.ts parsePrice",
  "exports src/shared/lib/index.ts parseProbability",
  "exports src/shared/lib/index.ts parseQuantity",
  "exports src/shared/lib/index.ts parseRatio",
  "exports src/shared/lib/index.ts parseTicker",
  "exports src/shared/lib/index.ts parseTimestamp",
  "exports src/shared/lib/index.ts text",
  "exports src/shared/lib/messages.ts text",
  "exports src/shared/lib/money.ts parseMoney",
  "exports src/shared/lib/money.ts parsePrice",
  "exports src/shared/lib/money.ts parseProbability",
  "exports src/shared/lib/money.ts parseQuantity",
  "exports src/shared/lib/money.ts parseRatio",
  "exports src/shared/platform/index.ts inputCommit",
  "exports src/shared/platform/index.ts inputEventValue",
  "exports src/shared/platform/index.ts pasteInputValue",
  "files src/shared/ds/Base.stories.tsx src/shared/ds/Base.stories.tsx",
  "files src/shared/ds/Button.stories.tsx src/shared/ds/Button.stories.tsx",
  "files src/shared/ds/Button.tsx src/shared/ds/Button.tsx",
  "files src/shared/ds/Checkbox.stories.tsx src/shared/ds/Checkbox.stories.tsx",
  "files src/shared/ds/Checkbox.tsx src/shared/ds/Checkbox.tsx",
  "files src/shared/ds/Disclosure.stories.tsx src/shared/ds/Disclosure.stories.tsx",
  "files src/shared/ds/Disclosure.tsx src/shared/ds/Disclosure.tsx",
  "files src/shared/ds/Heading.stories.tsx src/shared/ds/Heading.stories.tsx",
  "files src/shared/ds/Heading.tsx src/shared/ds/Heading.tsx",
  "files src/shared/ds/Icon.stories.tsx src/shared/ds/Icon.stories.tsx",
  "files src/shared/ds/Icon.tsx src/shared/ds/Icon.tsx",
  "files src/shared/ds/IconButton.stories.tsx src/shared/ds/IconButton.stories.tsx",
  "files src/shared/ds/IconButton.tsx src/shared/ds/IconButton.tsx",
  "files src/shared/ds/SegmentedControl.stories.tsx src/shared/ds/SegmentedControl.stories.tsx",
  "files src/shared/ds/SegmentedControl.tsx src/shared/ds/SegmentedControl.tsx",
  "files src/shared/ds/Select.stories.tsx src/shared/ds/Select.stories.tsx",
  "files src/shared/ds/Select.tsx src/shared/ds/Select.tsx",
  "files src/shared/ds/Switch.stories.tsx src/shared/ds/Switch.stories.tsx",
  "files src/shared/ds/Switch.tsx src/shared/ds/Switch.tsx",
  "files src/shared/ds/Text.stories.tsx src/shared/ds/Text.stories.tsx",
  "files src/shared/ds/Text.tsx src/shared/ds/Text.tsx",
  "files src/shared/ds/TextField.stories.tsx src/shared/ds/TextField.stories.tsx",
  "files src/shared/ds/TextField.tsx src/shared/ds/TextField.tsx",
  "files src/shared/ds/button-variants.tsx src/shared/ds/button-variants.tsx",
  "files src/shared/ds/control-types.ts src/shared/ds/control-types.ts",
  "files src/shared/ds/icon-paths-0.tsx src/shared/ds/icon-paths-0.tsx",
  "files src/shared/ds/icon-paths-1.tsx src/shared/ds/icon-paths-1.tsx",
  "files src/shared/ds/icon-paths-2.tsx src/shared/ds/icon-paths-2.tsx",
  "files src/shared/ds/icon-paths.tsx src/shared/ds/icon-paths.tsx",
  "files src/shared/ds/story-data.ts src/shared/ds/story-data.ts",
  "types src/shared/lib/brand.ts Count",
  "types src/shared/lib/brand.ts EntityId",
  "types src/shared/lib/brand.ts Ticker",
  "types src/shared/lib/brand.ts Timestamp",
  "types src/shared/lib/index.ts Count",
  "types src/shared/lib/index.ts Currency",
  "types src/shared/lib/index.ts DecimalParseError",
  "types src/shared/lib/index.ts EntityId",
  "types src/shared/lib/index.ts Err",
  "types src/shared/lib/index.ts Locale",
  "types src/shared/lib/index.ts MessageKey",
  "types src/shared/lib/index.ts Money",
  "types src/shared/lib/index.ts Ok",
  "types src/shared/lib/index.ts Price",
  "types src/shared/lib/index.ts Probability",
  "types src/shared/lib/index.ts Quantity",
  "types src/shared/lib/index.ts Ratio",
  "types src/shared/lib/index.ts Ticker",
  "types src/shared/lib/index.ts Timestamp",
  "types src/shared/lib/index.ts UiText",
  "types src/shared/lib/messages.ts Locale",
  "types src/shared/lib/messages.ts MessageKey",
  "types src/shared/lib/money.ts Currency",
  "types src/shared/lib/money.ts DecimalParseError",
  "types src/shared/lib/money.ts Money",
  "types src/shared/lib/money.ts Price",
  "types src/shared/lib/money.ts Probability",
  "types src/shared/lib/money.ts Quantity",
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
