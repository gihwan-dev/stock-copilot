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
  "dependencies package.json zod",
  "files src/shared/ds/index.ts src/shared/ds/index.ts",
  "files src/shared/ipc/index.ts src/shared/ipc/index.ts",
  "files src/shared/lib/index.ts src/shared/lib/index.ts",
  "files src/shared/lib/brand.ts src/shared/lib/brand.ts",
  "files src/shared/lib/messages.ts src/shared/lib/messages.ts",
  "files src/shared/lib/money.ts src/shared/lib/money.ts",
  "files src/shared/lib/result.ts src/shared/lib/result.ts",
  "dependencies package.json @tauri-apps/api",
  "files src/shared/ipc/bindings.ts src/shared/ipc/bindings.ts",
  "files src/shared/ipc/call.ts src/shared/ipc/call.ts",
  "files src/shared/ipc/schema.ts src/shared/ipc/schema.ts",
  "files src/shared/ipc/testing/fakes.ts src/shared/ipc/testing/fakes.ts",
  "files src/shared/ipc/testing/mockBindings.ts src/shared/ipc/testing/mockBindings.ts",
  "files src/shared/ipc/testing/responses.ts src/shared/ipc/testing/responses.ts",
  "files src/shared/ds/tokens.ts src/shared/ds/tokens.ts",
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
