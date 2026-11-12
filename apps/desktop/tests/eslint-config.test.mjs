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
  "guard/*",
  "@typescript-eslint/ban-ts-comment",
  "@eslint-community/eslint-comments/*",
];

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
