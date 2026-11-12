import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, relative } from "node:path";
import { test } from "node:test";
import {
  checkSteps,
  globMatches,
  listProblems,
  readSteps,
  runnerGlobs,
} from "../../../harness/check-list/list.mjs";
import { repoRoot } from "./paths.mjs";

const justfile = readFileSync(join(repoRoot, "justfile"), "utf8");
const list = readSteps(repoRoot);

test("the harness check list holds exactly the steps just check runs, each with its examples and self test", () => {
  assert.deepEqual(listProblems(repoRoot, list, justfile), []);
  assert.equal(list.steps.length, checkSteps(justfile).length);
});

test("dropping a step from just check or running an unlisted one fails the list", () => {
  const dropped = justfile.replace(
    '    "{{just_executable()}}" sql-check\n',
    "",
  );
  assert.deepEqual(listProblems(repoRoot, list, dropped), [
    '목록에 있지만 just check가 돌지 않는다: check | "{{just_executable()}}" sql-check',
  ]);
  const added = justfile.replace(
    "    cargo fmt --all --check\n",
    "    cargo fmt --all --check\n    true\n",
  );
  assert.deepEqual(listProblems(repoRoot, list, added), [
    "just check가 돌지만 목록에 없다: check | true",
  ]);
  const weakened = justfile.replace(
    "    node harness/knip-check/check.mjs\n",
    "    node harness/knip-check/check.mjs || true\n",
  );
  assert.equal(listProblems(repoRoot, list, weakened).length, 2);
});

const withoutIndex = (items, index) => [
  ...items.slice(0, index),
  ...items.slice(index + 1),
];

const withoutKey = (object, key) =>
  Object.fromEntries(Object.entries(object).filter(([other]) => other !== key));

const variants = (steps) =>
  steps.flatMap((step, index) => [
    { name: `steps.${index}`, steps: withoutIndex(steps, index) },
    ...Object.keys(step).map((key) => ({
      name: `steps.${index}.${key}`,
      steps: steps.map((other, position) =>
        position === index ? withoutKey(other, key) : other,
      ),
    })),
    ...Object.keys(step.test).map((key) => ({
      name: `steps.${index}.test.${key}`,
      steps: steps.map((other, position) =>
        position === index
          ? { ...other, test: withoutKey(other.test, key) }
          : other,
      ),
    })),
  ]);

test("removing any element of the check list makes the list fail", () => {
  const surviving = variants(list.steps)
    .filter(
      (variant) =>
        listProblems(repoRoot, { steps: variant.steps }, justfile).length === 0,
    )
    .map((variant) => variant.name);
  assert.deepEqual(surviving, []);
});

test("a missing self test, example location or unknown kind fails the list", () => {
  const broken = (change) =>
    listProblems(
      repoRoot,
      {
        steps: list.steps.map((step, index) =>
          index === 0 ? change(step) : step,
        ),
      },
      justfile,
    );
  assert.equal(
    broken((step) => ({
      ...step,
      test: { ...step.test, name: "no such test" },
    })).length,
    1,
  );
  assert.equal(
    broken((step) => ({ ...step, cases: "harness/no-such-cases" })).length,
    1,
  );
  assert.equal(broken((step) => ({ ...step, kind: "other" })).length, 1);
  assert.equal(
    broken((step) => ({
      ...step,
      test: { file: "crates/xtask/src/test_runner_tests.rs", name: "check" },
    })).length,
    1,
  );
});

const testFiles = (directory) =>
  readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      return ["node_modules", "target", ".git"].includes(entry.name)
        ? []
        : testFiles(path);
    }
    return entry.name.endsWith(".test.mjs") ? [relative(repoRoot, path)] : [];
  });

test("the node test runners of just check reach every test file", () => {
  const globs = runnerGlobs(justfile);
  assert.ok(globs.length >= 2);
  const files = testFiles(repoRoot);
  assert.ok(files.includes("apps/desktop/tests/check-list.test.mjs"));
  assert.deepEqual(
    files.filter((file) => !globs.some((glob) => globMatches(glob, file))),
    [],
  );
  globs.forEach((glob) =>
    assert.ok(
      files.some((file) => globMatches(glob, file)),
      `${glob} matches no test file`,
    ),
  );
});

test("just check installs the frozen lockfile without lifecycle scripts before any other Node step", () => {
  const commands = checkSteps(justfile).map((step) => step.command);
  const install = commands.indexOf(
    "pnpm install --frozen-lockfile --ignore-scripts",
  );
  const firstNode = commands.findIndex(
    (command) =>
      /^(node|pnpm|apps\/desktop\/node_modules)/u.test(command) &&
      command !== "pnpm install --frozen-lockfile --ignore-scripts",
  );
  assert.ok(install >= 0 && install < firstNode);
});

const tscCopy = () => {
  const root = mkdtempSync(join(tmpdir(), "tsc-violations-"));
  mkdirSync(join(root, "harness"), { recursive: true });
  mkdirSync(join(root, "apps"), { recursive: true });
  cpSync(
    join(repoRoot, "harness/tsc-violations"),
    join(root, "harness/tsc-violations"),
    { recursive: true },
  );
  symlinkSync(join(repoRoot, "apps/desktop"), join(root, "apps/desktop"));
  return root;
};

const runTscCheck = (root) =>
  spawnSync(
    process.execPath,
    [join(root, "harness/tsc-violations/check.mjs")],
    { encoding: "utf8" },
  );

test("a tsc violation case that stops failing makes the tsc-violations check fail", () => {
  const expected = JSON.parse(
    readFileSync(
      join(repoRoot, "harness/tsc-violations/expected.json"),
      "utf8",
    ),
  );
  const [caseFile] = Object.entries(expected.cases).find(
    ([file, codes]) =>
      file.startsWith("cases/") && Array.isArray(codes) && codes.length > 0,
  );
  const root = tscCopy();
  try {
    const untouched = runTscCheck(root);
    assert.equal(untouched.status, 0, untouched.stdout);
    writeFileSync(
      join(root, "harness/tsc-violations", caseFile),
      "export const neutral = 1 as const;\n",
    );
    const neutralized = runTscCheck(root);
    assert.equal(neutralized.status, 1, neutralized.stdout);
    assert.match(neutralized.stdout, new RegExp(`FAIL .*${caseFile}`, "u"));
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
