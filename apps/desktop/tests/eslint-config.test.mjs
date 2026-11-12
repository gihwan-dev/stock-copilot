import { strict as assert } from "node:assert";
import { readdirSync, readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join, relative, sep } from "node:path";
import { test } from "node:test";
import { isDeepStrictEqual } from "node:util";
import {
  readCases,
  violationLinter,
} from "../../../harness/eslint-violations/lint.mjs";
import { desktopRoot, repoRoot } from "./paths.mjs";

const requireFromDesktop = createRequire(join(desktopRoot, "package.json"));
const { ESLint } = requireFromDesktop("eslint");
const appLinter = new ESLint({ cwd: desktopRoot, cache: false });
const { cases } = readCases();
const manifest = JSON.parse(
  readFileSync(join(desktopRoot, "package.json"), "utf8"),
);

const severityOf = (entry) => (Array.isArray(entry) ? entry[0] : entry);

const walk = (folder) =>
  readdirSync(folder, { withFileTypes: true }).flatMap((entry) =>
    entry.isDirectory()
      ? walk(join(folder, entry.name))
      : [join(folder, entry.name)],
  );

const representativeFiles = [
  "src/app/main.tsx",
  "src/features/home/ui/HomeScreen.tsx",
  "src/features/home/model/queries.ts",
  "src/shared/ds/Button.tsx",
  "src/shared/ipc/client.ts",
  "src/shared/ipc/testing/mocks.ts",
  "src/shared/lib/result.ts",
  "src/shared/lib/money.ts",
  "src/shared/platform/useCoreEvent.ts",
  "src/shared/platform/unlisted.ts",
];

const forbiddenFiles = [
  "src/features/home/legacy.js",
  "src/features/home/legacy.jsx",
  "src/features/home/legacy.mjs",
  "src/features/home/legacy.cjs",
  "src/features/home/legacy.mts",
  "src/features/home/legacy.cts",
  "src/shared/lib/ambient.d.ts",
];

const undisableable = [
  "local/no-comments",
  "local/no-money-coercion",
  "local/forbidden-file",
  "boundaries/*",
  "guard/no-effect-hooks",
  "guard/no-sync-external-store",
  "guard/no-ref-callback",
  "guard/no-type-predicate",
  "guard/no-number-conversion",
  "guard/no-unary-plus",
  "guard/no-reflect",
  "guard/no-prototype",
  "guard/no-call-apply-bind",
  "guard/no-own-symbols",
  "guard/no-network",
  "guard/no-global-object",
  "guard/no-test-skip",
  "guard/forbidden-extension",
  "guard/no-dom-globals",
  "guard/disable-directive-form",
  "@typescript-eslint/ban-ts-comment",
  "@eslint-community/eslint-comments/no-use",
  "@eslint-community/eslint-comments/require-description",
  "@eslint-community/eslint-comments/no-unlimited-disable",
  "@eslint-community/eslint-comments/no-restricted-disable",
  "@eslint-community/eslint-comments/no-duplicate-disable",
];

const severityIn = async (file, rule) =>
  severityOf(
    (await appLinter.calculateConfigForFile(join(desktopRoot, file))).rules[
      rule
    ] ?? 0,
  );

test("lint tools are pinned to the versions of harness 4.3 and R-08", () => {
  assert.deepEqual(
    {
      eslint: manifest.devDependencies.eslint,
      "typescript-eslint": manifest.devDependencies["typescript-eslint"],
      "eslint-plugin-functional":
        manifest.devDependencies["eslint-plugin-functional"],
      "@eslint/js": manifest.devDependencies["@eslint/js"],
      "eslint-plugin-react-hooks":
        manifest.devDependencies["eslint-plugin-react-hooks"],
      "@eslint-react/eslint-plugin":
        manifest.devDependencies["@eslint-react/eslint-plugin"],
      "@eslint-community/eslint-plugin-eslint-comments":
        manifest.devDependencies[
          "@eslint-community/eslint-plugin-eslint-comments"
        ],
    },
    {
      eslint: "10.14.0",
      "typescript-eslint": "8.72.2",
      "eslint-plugin-functional": "10.0.1",
      "@eslint/js": "10.0.1",
      "eslint-plugin-react-hooks": "7.1.1",
      "@eslint-react/eslint-plugin": "5.30.1",
      "@eslint-community/eslint-plugin-eslint-comments": "4.8.1",
    },
  );
  assert.equal(ESLint.version, "10.14.0");
});

test("typescript-eslint parses with the TypeScript 6 API (I-16)", () => {
  const estree = createRequire(
    requireFromDesktop.resolve("typescript-eslint"),
  ).resolve("@typescript-eslint/typescript-estree");
  const ts = createRequire(estree)("typescript");
  assert.match(ts.version, /^6\.0\.\d+$/);
});

test("violation cases are linted with exactly the app configuration for the same path", async () => {
  const examples = violationLinter(undefined);
  const differing = [];
  for (const file of Object.keys(cases)) {
    const app = await appLinter.calculateConfigForFile(join(desktopRoot, file));
    const example = await examples.calculateConfigForFile(file);
    if (
      !isDeepStrictEqual(example.rules, app.rules) ||
      !isDeepStrictEqual(example.linterOptions, app.linterOptions)
    )
      differing.push(file);
  }
  assert.deepEqual(differing, []);
});

test("no rule is left at warning level in the effective configuration", async () => {
  const warnings = [];
  for (const file of [...representativeFiles, ...forbiddenFiles]) {
    const config = await appLinter.calculateConfigForFile(
      join(desktopRoot, file),
    );
    Object.entries(config.rules)
      .filter(([, entry]) => severityOf(entry) === 1)
      .forEach(([rule]) => warnings.push(`${file} ${rule}`));
  }
  assert.deepEqual(warnings, []);
});

test("unused directives and inline configs are errors and inline disables stay possible only in TypeScript", async () => {
  for (const file of representativeFiles) {
    const config = await appLinter.calculateConfigForFile(
      join(desktopRoot, file),
    );
    assert.equal(config.linterOptions.reportUnusedDisableDirectives, 2, file);
    assert.equal(config.linterOptions.reportUnusedInlineConfigs, 2, file);
    assert.equal(config.linterOptions.noInlineConfig, false, file);
  }
});

test("rules of harness 2.1 that cannot be disabled are listed in no-restricted-disable", async () => {
  for (const file of representativeFiles) {
    const config = await appLinter.calculateConfigForFile(
      join(desktopRoot, file),
    );
    const entry =
      config.rules["@eslint-community/eslint-comments/no-restricted-disable"];
    assert.equal(severityOf(entry), 2, file);
    assert.deepEqual(entry.slice(1), undisableable, file);
    assert.deepEqual(
      config.rules["@eslint-community/eslint-comments/no-use"],
      [2, { allow: ["eslint-disable-next-line"] }],
      file,
    );
    assert.equal(severityOf(config.rules["guard/disable-directive-form"]), 2);
  }
});

test("every undisableable guard rule exists and no rule family is undisableable by a glob except boundaries", async () => {
  const config = await appLinter.calculateConfigForFile(
    join(desktopRoot, "src/features/home/ui/HomeScreen.tsx"),
  );
  const guardRules = undisableable.filter((rule) => rule.startsWith("guard/"));
  const forbidden = await appLinter.calculateConfigForFile(
    join(desktopRoot, "src/features/home/legacy.js"),
  );
  assert.deepEqual(
    guardRules.filter(
      (rule) =>
        config.rules[rule] === undefined && forbidden.rules[rule] === undefined,
    ),
    [],
  );
  assert.deepEqual(
    undisableable.filter(
      (rule) => rule.includes("*") && rule !== "boundaries/*",
    ),
    [],
  );
});

test("browser globals are banned outside shared/ds and the platform allowlist", async () => {
  const banned = [
    "src/app/main.tsx",
    "src/features/home/ui/HomeScreen.tsx",
    "src/shared/lib/result.ts",
    "src/shared/ipc/client.ts",
    "src/shared/platform/unlisted.ts",
  ];
  const allowed = ["src/shared/ds/Button.tsx", "src/shared/platform/index.ts"];
  for (const file of banned)
    assert.equal(await severityIn(file, "guard/no-dom-globals"), 2, file);
  for (const file of allowed)
    assert.equal(await severityIn(file, "guard/no-dom-globals"), 0, file);
});

test("number globals are banned in features, app and shared/ds only", async () => {
  const banned = [
    "src/app/main.tsx",
    "src/features/home/ui/HomeScreen.tsx",
    "src/shared/ds/Button.tsx",
  ];
  const allowed = [
    "src/shared/lib/result.ts",
    "src/shared/ipc/client.ts",
    "src/shared/platform/index.ts",
  ];
  for (const file of banned)
    assert.equal(await severityIn(file, "guard/no-number-conversion"), 2, file);
  for (const file of allowed)
    assert.equal(await severityIn(file, "guard/no-number-conversion"), 0, file);
});

test("future test files under src get exactly the app configuration", async () => {
  const pairs = [
    [
      "src/features/home/model/queries.ts",
      "src/features/home/model/queries.test.ts",
    ],
    [
      "src/features/home/ui/HomeScreen.tsx",
      "src/features/home/ui/HomeScreen.test.tsx",
    ],
  ];
  for (const [source, testFile] of pairs) {
    const a = await appLinter.calculateConfigForFile(join(desktopRoot, source));
    const b = await appLinter.calculateConfigForFile(
      join(desktopRoot, testFile),
    );
    assert.ok(isDeepStrictEqual(a.rules, b.rules), testFile);
  }
});

test("forbidden source extensions are linted, fail, and cannot carry inline directives", async () => {
  for (const file of forbiddenFiles) {
    const path = join(desktopRoot, file);
    assert.equal(await appLinter.isPathIgnored(path), false, file);
    const config = await appLinter.calculateConfigForFile(path);
    assert.notEqual(config, undefined, file);
    assert.equal(
      severityOf(config.rules["guard/forbidden-extension"]),
      2,
      file,
    );
    assert.equal(config.linterOptions.noInlineConfig, true, file);
  }
});

test("every file under apps/desktop/src is a TypeScript lint target", async () => {
  const outside = [];
  for (const path of walk(join(desktopRoot, "src"))) {
    const file = relative(desktopRoot, path).split(sep).join("/");
    const config = (await appLinter.isPathIgnored(path))
      ? undefined
      : await appLinter.calculateConfigForFile(path);
    const typescriptTarget =
      config !== undefined &&
      severityOf(config.rules["@typescript-eslint/no-explicit-any"]) === 2 &&
      config.rules["guard/forbidden-extension"] === undefined;
    if (!file.endsWith("/.gitkeep") && !typescriptTarget) outside.push(file);
  }
  assert.deepEqual(outside, []);
});

test("just check runs ESLint on src without cache and with zero warnings allowed", () => {
  const justfile = readFileSync(join(repoRoot, "justfile"), "utf8");
  assert.match(
    justfile,
    /^ {4}pnpm -C apps\/desktop exec eslint --max-warnings 0 src$/m,
  );
  assert.match(justfile, /^ {4}node harness\/eslint-violations\/check\.mjs$/m);
  assert.doesNotMatch(justfile, /--cache/);
});

const scriptLinter = new ESLint({
  cwd: repoRoot,
  overrideConfigFile: join(desktopRoot, "eslint.scripts.config.mjs"),
  cache: false,
});

const scriptRules = async (path, code) => {
  const [result] = await scriptLinter.lintText(code, {
    filePath: join(repoRoot, path),
  });
  return [...new Set(result.messages.map((m) => m.ruleId))].sort();
};

test("test and harness scripts fail on skipped or focused tests", async () => {
  const cases = [
    [
      "apps/desktop/tests/a.test.mjs",
      'import { test } from "node:test";\ntest.skip("x", () => {});\n',
    ],
    [
      "apps/desktop/tests/a.test.mjs",
      'import { test } from "node:test";\ntest("x", { only: true }, () => {});\n',
    ],
    [
      "harness/x/check.mjs",
      'import { describe } from "node:test";\nconst { todo } = describe;\nexport { todo };\n',
    ],
  ];
  for (const [path, code] of cases)
    assert.deepEqual(
      await scriptRules(path, code),
      ["guard/no-test-skip"],
      code,
    );
});

test("test and harness scripts allow only the 2.1 directive form", async () => {
  assert.deepEqual(
    await scriptRules(
      "harness/x/check.mjs",
      "/* eslint-disable */\nexport const a = 1;\n",
    ),
    [
      "@eslint-community/eslint-comments/no-restricted-disable",
      "@eslint-community/eslint-comments/no-unlimited-disable",
      "@eslint-community/eslint-comments/no-use",
      "@eslint-community/eslint-comments/require-description",
    ],
  );
  assert.deepEqual(
    await scriptRules(
      "harness/x/check.mjs",
      "/* eslint-disable-next-line no-debugger -- 블록 */\ndebugger;\n",
    ),
    ["guard/disable-directive-form"],
  );
  assert.deepEqual(
    await scriptRules(
      "apps/desktop/tests/a.test.mjs",
      "export const a = () => {\n  // eslint-disable-next-line no-debugger -- 예제\n  debugger;\n};\n",
    ),
    [],
  );
});

test("test and harness scripts get JS recommended rules", async () => {
  assert.deepEqual(
    await scriptRules("harness/x/check.mjs", "export const a = b;\n"),
    ["no-undef"],
  );
});

test("violation examples are not linted as scripts", async () => {
  assert.equal(
    await scriptLinter.isPathIgnored(
      join(repoRoot, "harness/eslint-violations/src/features/files/legacy.js"),
    ),
    true,
  );
});

test("just check lints test and harness scripts", () => {
  const justfile = readFileSync(join(repoRoot, "justfile"), "utf8");
  assert.match(
    justfile,
    /^ {4}apps\/desktop\/node_modules\/\.bin\/eslint --max-warnings 0 --config apps\/desktop\/eslint\.scripts\.config\.mjs apps\/desktop\/tests apps\/desktop\/eslint apps\/desktop\/eslint\.config\.mjs apps\/desktop\/eslint\.scripts\.config\.mjs harness$/m,
  );
});
