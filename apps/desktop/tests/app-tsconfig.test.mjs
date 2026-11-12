import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { appConfigProblems } from "../../../harness/tsc-violations/app-config.mjs";
import { desktopRoot, repoRoot } from "./paths.mjs";

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
const appConfig = readJson(join(desktopRoot, "tsconfig.json"));
const expected = readJson(
  join(repoRoot, "harness", "tsc-violations", "expected.json"),
);

test("the app tsconfig matches the expected config exactly", () => {
  assert.deepEqual(appConfigProblems(appConfig, expected), []);
});

test("the expected config includes only src", () => {
  assert.deepEqual(expected.include, ["src"]);
});

test("adding exclude to the app tsconfig is a problem", () => {
  const problems = appConfigProblems(
    { ...appConfig, exclude: ["src/features"] },
    expected,
  );
  assert.equal(problems.length, 1);
  assert.match(problems[0], /top-level keys/);
});

test("adding files to the app tsconfig is a problem", () => {
  const problems = appConfigProblems(
    { ...appConfig, files: ["src/app/main.tsx"] },
    expected,
  );
  assert.equal(problems.length, 1);
  assert.match(problems[0], /top-level keys/);
});

test("adding extends to the app tsconfig is a problem", () => {
  const problems = appConfigProblems(
    { ...appConfig, extends: "./base.json" },
    expected,
  );
  assert.equal(problems.length, 1);
  assert.match(problems[0], /top-level keys/);
});

test("changing include in the app tsconfig is a problem", () => {
  const problems = appConfigProblems(
    { ...appConfig, include: ["src/app"] },
    expected,
  );
  assert.equal(problems.length, 1);
  assert.match(problems[0], /^include:/);
});

test("an option outside the expected list is a problem", () => {
  const problems = appConfigProblems(
    {
      ...appConfig,
      compilerOptions: { ...appConfig.compilerOptions, rootDir: "src/app" },
    },
    expected,
  );
  assert.deepEqual(problems, [
    'option rootDir: expected undefined, got "src/app"',
  ]);
});

test("changing a non-strictness option like target is a problem", () => {
  const problems = appConfigProblems(
    {
      ...appConfig,
      compilerOptions: { ...appConfig.compilerOptions, target: "es5" },
    },
    expected,
  );
  assert.deepEqual(problems, ['option target: expected "es2024", got "es5"']);
});

test("removing a harness 4.1 option is a problem", () => {
  const { noUnusedLocals: _, ...rest } = appConfig.compilerOptions;
  const problems = appConfigProblems(
    { ...appConfig, compilerOptions: rest },
    expected,
  );
  assert.deepEqual(problems, [
    "option noUnusedLocals: expected true, got undefined",
  ]);
});
