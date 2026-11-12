import { strict as assert } from "node:assert";
import { createRequire } from "node:module";
import { join } from "node:path";
import { test } from "node:test";
import {
  allowlistLinter,
  appAllowlists,
  lintCases,
  localRuleMessages,
  readCases,
} from "../../../harness/eslint-violations/lint.mjs";
import { caseProblems } from "../../../harness/eslint-violations/verdict.mjs";
import { desktopRoot } from "./paths.mjs";

const { cases } = readCases();
const allFiles = Object.keys(cases);

const expectedLists = {
  PLATFORM_FILE_ALLOWLIST: [
    "src/shared/platform/index.ts",
    "src/shared/platform/detach.ts",
    "src/shared/platform/useCoreEvent.ts",
    "src/shared/platform/useDocumentTitle.ts",
  ],
  TYPE_PREDICATE_ALLOWLIST: ["src/shared/lib/money.ts"],
  THROW_ALLOWLIST: [],
  CHART_NUMBER_ALLOWLIST: [],
  RAW_TYPE_ALLOWLIST: [
    "src/shared/lib/brand.ts",
    "src/shared/lib/money.ts",
    "src/shared/lib/format.ts",
    "src/shared/ipc/schema.ts",
    "src/shared/ipc/bindings.ts",
  ],
  PROMISE_REJECTION_ALLOWLIST: [
    "src/shared/ipc/call.ts",
    "src/shared/platform/detach.ts",
  ],
  MONEY_FILE: "src/shared/lib/money.ts",
  MONEY_KEY_READERS: ["src/shared/lib/format.ts"],
  MESSAGES_FILES: ["src/**/messages.ts"],
  FILE_PLACEHOLDERS: [".gitkeep"],
};

const withList = (name, value) => ({ ...appAllowlists, [name]: value });

const failingWith = async (lists) => {
  const linted = await lintCases(allFiles, undefined, allowlistLinter(lists));
  return allFiles.filter(
    (file) =>
      caseProblems(cases[file].expect, linted.get(file)?.keys ?? []).length > 0,
  );
};

test("the file allowlists of harness 4.2 hold exactly the documented entries", () => {
  assert.deepEqual({ ...appAllowlists }, expectedLists);
});

test("the unchanged allowlists pass every case through the allowlist builder", async () => {
  assert.deepEqual(await failingWith({ ...appAllowlists }), []);
});

test("removing any single allowlist entry makes some case fail", async () => {
  const variants = Object.entries(expectedLists)
    .filter(([, value]) => Array.isArray(value))
    .flatMap(([name, entries]) =>
      entries.map((entry) => ({
        label: `${name} - ${entry}`,
        lists: withList(
          name,
          entries.filter((other) => other !== entry),
        ),
      })),
    );
  const survivors = [];
  for (const { label, lists } of variants) {
    if ((await failingWith(lists)).length === 0) survivors.push(label);
  }
  assert.deepEqual(survivors, []);
});

test("adding a file to any allowlist makes some case fail", async () => {
  const additions = [
    ["PLATFORM_FILE_ALLOWLIST", "src/shared/platform/unlisted.ts"],
    ["TYPE_PREDICATE_ALLOWLIST", "src/shared/lib/unlisted-predicate.ts"],
    ["THROW_ALLOWLIST", "src/app/failure/throw.ts"],
    ["CHART_NUMBER_ALLOWLIST", "src/shared/ds/numbers/number-call.ts"],
    ["RAW_TYPE_ALLOWLIST", "src/features/raw/string-param.ts"],
    ["PROMISE_REJECTION_ALLOWLIST", "src/features/promise/catch.ts"],
    ["MONEY_KEY_READERS", "src/features/money/computed.ts"],
    ["MESSAGES_FILES", "src/app/text/hangul.ts"],
  ];
  const survivors = [];
  for (const [name, file] of additions) {
    const lists = withList(name, [...appAllowlists[name], file]);
    if ((await failingWith(lists)).length === 0)
      survivors.push(`${name} + ${file}`);
  }
  assert.deepEqual(survivors, []);
});

test("moving the money file makes the money cases fail", async () => {
  const failing = await failingWith(
    withList("MONEY_FILE", "src/shared/lib/format.ts"),
  );
  assert.ok(failing.includes("src/features/money/compare.ts"));
});

test("turning off any local rule makes exactly its cases fail", async () => {
  const survivors = [];
  for (const rule of Object.keys(localRuleMessages)) {
    const files = allFiles.filter(
      (file) =>
        Array.isArray(cases[file].expect) &&
        cases[file].expect.some((key) => key.startsWith(`${rule} :: `)),
    );
    assert.notDeepEqual(files, [], rule);
    const linted = await lintCases(files, {
      files,
      rules: { [rule]: "off" },
    });
    files
      .filter(
        (file) =>
          caseProblems(cases[file].expect, linted.get(file)?.keys ?? [])
            .length === 0,
      )
      .forEach((file) => survivors.push(`${rule} ${file}`));
  }
  assert.deepEqual(survivors, []);
});

test("every message of every local rule is expected by some case", () => {
  const expected = new Set(
    Object.values(cases).flatMap((spec) =>
      Array.isArray(spec.expect) ? spec.expect : [],
    ),
  );
  const missing = Object.entries(localRuleMessages).flatMap(
    ([rule, messages]) =>
      messages
        .map((message) => `${rule} :: ${message}`)
        .filter((key) => !expected.has(key)),
  );
  assert.deepEqual(missing, []);
});

test("local rules carry the documented severity and mode for each kind of file", async () => {
  const { ESLint } = createRequire(join(desktopRoot, "package.json"))("eslint");
  const app = new ESLint({ cwd: desktopRoot, cache: false });
  const rulesOf = async (file) =>
    (await app.calculateConfigForFile(join(desktopRoot, file))).rules;
  const table = [
    [
      "src/features/home/ui/HomeScreen.tsx",
      "local/no-ui-text",
      [2, { screen: "on" }],
    ],
    ["src/app/main.tsx", "local/no-ui-text", [2, { screen: "on" }]],
    ["src/shared/ds/Button.tsx", "local/no-ui-text", [2, { screen: "on" }]],
    ["src/shared/lib/result.ts", "local/no-ui-text", [2, { screen: "off" }]],
    ["src/shared/lib/messages.ts", "local/no-ui-text", [0]],
    [
      "src/features/home/messages.ts",
      "local/no-brand-creation",
      [2, { mode: "messages" }],
    ],
    [
      "src/shared/lib/brand.ts",
      "local/no-brand-creation",
      [2, { mode: "brand-file" }],
    ],
    [
      "src/shared/lib/result.ts",
      "local/no-brand-creation",
      [2, { mode: "forbid" }],
    ],
    ["src/shared/lib/brand.ts", "local/no-raw-types", [0]],
    ["src/shared/lib/result.ts", "local/no-raw-types", [2]],
    ["src/shared/ipc/call.ts", "guard/no-promise-catch", [0]],
    ["src/shared/ipc/client.ts", "guard/no-promise-catch", [2]],
    ["src/shared/lib/result.ts", "local/no-comments", [2]],
    [
      "src/features/home/ui/HomeScreen.tsx",
      "local/no-money-coercion",
      [
        2,
        {
          moneyFile: "src/shared/lib/money.ts",
          keyReaders: ["src/shared/lib/format.ts"],
        },
      ],
    ],
  ];
  const wrong = [];
  for (const [file, rule, want] of table) {
    const entry = (await rulesOf(file))[rule] ?? [0];
    const full = Array.isArray(entry) ? entry : [entry];
    const got = want.length === 1 ? full.slice(0, 1) : full;
    if (JSON.stringify(got) !== JSON.stringify(want))
      wrong.push(`${file} ${rule} ${JSON.stringify(got)}`);
  }
  assert.deepEqual(wrong, []);
});

test("files outside .ts and .tsx under src get the extension check and no inline config", async () => {
  const { ESLint } = createRequire(join(desktopRoot, "package.json"))("eslint");
  const app = new ESLint({ cwd: desktopRoot, cache: false });
  for (const file of [
    "src/features/x.TSX",
    "src/features/x.JS",
    "src/shared/ds/tokens.css",
    "src/features/README",
  ]) {
    const path = join(desktopRoot, file);
    assert.equal(await app.isPathIgnored(path), false, file);
    const config = await app.calculateConfigForFile(path);
    assert.deepEqual(
      config.rules["local/forbidden-file"][1].checkExtension,
      true,
      file,
    );
    assert.equal(config.linterOptions.noInlineConfig, true, file);
  }
});
