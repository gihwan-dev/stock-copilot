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
  dumpJustfile,
  globMatches,
  justfileProblems,
  listProblems,
  readExpectedJustfile,
  readSteps,
  readsCases,
  runnableRustTests,
  runnerGlobs,
} from "../../../harness/check-list/list.mjs";
import { repoRoot } from "./paths.mjs";

const justfileText = readFileSync(join(repoRoot, "justfile"), "utf8");
const justfile = dumpJustfile(join(repoRoot, "justfile"));
const expectedJustfile = readExpectedJustfile(repoRoot);
const list = readSteps(repoRoot);
const runnable = runnableRustTests(repoRoot);

const scratch = (prefix) => mkdtempSync(join(tmpdir(), prefix));

const dumpOf = (text, extra = {}) => {
  const directory = scratch("justfile-variant-");
  try {
    writeFileSync(join(directory, "justfile"), text);
    Object.entries(extra).forEach(([name, content]) =>
      writeFileSync(join(directory, name), content),
    );
    return dumpJustfile(join(directory, "justfile"));
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
};

test("just reads the justfile exactly as the protected expectation", () => {
  assert.equal(justfile.problem, undefined);
  assert.deepEqual(justfileProblems(justfile, expectedJustfile), []);
});

const replaced = (from, to) => {
  assert.ok(justfileText.includes(from), from);
  return justfileText.replace(from, to);
};

const VARIANTS = [
  [
    "a sub-recipe body becomes true",
    () =>
      replaced(
        "    cargo run --locked --quiet --package xtask -- sql-check\n",
        "    true\n",
      ),
  ],
  [
    "a sub-recipe body becomes echo",
    () =>
      replaced(
        "    cargo run --locked --quiet --package xtask -- raw-types\n",
        "    echo cargo run --locked --quiet --package xtask -- raw-types\n",
      ),
  ],
  [
    "a body line ends with || true",
    () =>
      replaced(
        "    cargo run --locked --quiet --package xtask -- disable-check\n",
        "    cargo run --locked --quiet --package xtask -- disable-check || true\n",
      ),
  ],
  [
    "the dependency order changes",
    () =>
      replaced(
        "check: check-node require-base check-ts",
        "check: check-ts check-node require-base",
      ),
  ],
  ["set ignore-errors", () => `set ignore-errors\n${justfileText}`],
  [
    "set shell",
    () =>
      replaced(
        'set shell := ["bash", "-euo", "pipefail", "-c"]',
        'set shell := ["bash", "-c"]',
      ),
  ],
  ["set dotenv-load", () => `set dotenv-load\n${justfileText}`],
  [
    "an import",
    () => `import 'extra.just'\n${justfileText}`,
    { "extra.just": "extra:\n    true\n" },
  ],
  [
    "a module",
    () => `mod extra\n${justfileText}`,
    { "extra.just": "extra:\n    true\n" },
  ],
  [
    "an environment variable skips a step",
    () =>
      replaced(
        "    cargo run --locked --quiet --package xtask -- migration-check\n",
        '    [ -n "${SKIP_CHECK:-}" ] || cargo run --locked --quiet --package xtask -- migration-check\n',
      ),
  ],
  [
    "a variable hides the command",
    () =>
      replaced(
        "    cargo run --locked --quiet --package xtask -- capability-check\n",
        "    {{capability}}\n",
      ).replace('base := ""', 'base := ""\ncapability := "true"'),
  ],
  [
    "if false wraps a command",
    () =>
      replaced(
        "    cargo run --locked --quiet --package xtask -- source-check\n",
        "    if false; then cargo run --locked --quiet --package xtask -- source-check; fi\n",
      ),
  ],
  [
    "no-exit-message",
    () => replaced("arch-check:\n", "[no-exit-message]\narch-check:\n"),
  ],
  ["a recipe is added", () => `${justfileText}\nextra:\n    true\n`],
];

test("every justfile change the review listed fails the justfile expectation", () => {
  const surviving = VARIANTS.filter(
    ([, make, extra]) =>
      justfileProblems(dumpOf(make(), extra), expectedJustfile).length === 0,
  ).map(([name]) => name);
  assert.deepEqual(surviving, []);
  assert.deepEqual(
    justfileProblems(dumpOf(justfileText), expectedJustfile),
    [],
  );
});

test("the harness check list holds exactly the steps just check runs, each with its examples and a running self test that reads them", () => {
  assert.equal(runnable.problem, undefined);
  assert.deepEqual(listProblems(repoRoot, list, justfile, runnable), []);
  assert.equal(list.steps.length, checkSteps(justfile).length);
});

test("dropping a step from just check or running an unlisted one fails the list", () => {
  const dropped = dumpOf(
    replaced('    "{{just_executable()}}" sql-check\n', ""),
  );
  assert.deepEqual(listProblems(repoRoot, list, dropped, runnable), [
    '목록에 있지만 just check가 돌지 않는다: check | "{{just_executable()}}" sql-check',
  ]);
  const added = dumpOf(
    replaced(
      "    cargo fmt --all --check\n",
      "    cargo fmt --all --check\n    true\n",
    ),
  );
  assert.deepEqual(listProblems(repoRoot, list, added, runnable), [
    "just check가 돌지만 목록에 없다: check | true",
  ]);
  const weakened = dumpOf(
    replaced(
      "    node harness/knip-check/check.mjs\n",
      "    node harness/knip-check/check.mjs || true\n",
    ),
  );
  assert.equal(listProblems(repoRoot, list, weakened, runnable).length, 2);
});

test("a self test that is ignored, not run or does not read the step's examples fails the list", () => {
  const arch = list.steps.find((step) => step.command.includes("arch-check"));
  assert.ok(readsCases(repoRoot, arch));
  const unrelated = {
    ...arch,
    test: {
      file: "crates/xtask/src/rust_lints_tests.rs",
      name: "rustflags_is_neutralized",
    },
  };
  assert.equal(readsCases(repoRoot, unrelated), false);
  const withStep = (step) => ({
    steps: list.steps.map((other) => (other === arch ? step : other)),
  });
  assert.equal(
    listProblems(repoRoot, withStep(unrelated), justfile, runnable).length,
    1,
  );
  const ignored = {
    tests: new Set(
      [...runnable.tests].filter(
        (id) =>
          id !==
          "arch_check_cases_tests::command_fails_on_violation_and_passes_clean_workspace",
      ),
    ),
  };
  assert.equal(listProblems(repoRoot, list, justfile, ignored).length, 1);
  const unrunNode = {
    ...arch,
    test: {
      file: "harness/check-list/list.mjs",
      name: "removing any element of the check list makes the list fail",
    },
  };
  assert.ok(
    listProblems(repoRoot, withStep(unrunNode), justfile, runnable).length >= 1,
  );
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
        listProblems(repoRoot, { steps: variant.steps }, justfile, runnable)
          .length === 0,
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
      runnable,
    ).length;
  assert.ok(
    broken((step) => ({
      ...step,
      test: { ...step.test, name: "no such test" },
    })) >= 1,
  );
  assert.ok(
    broken((step) => ({ ...step, cases: "harness/no-such-cases" })) >= 1,
  );
  assert.ok(broken((step) => ({ ...step, kind: "other" })) >= 1);
  assert.ok(
    broken((step) => ({
      ...step,
      test: { file: "crates/xtask/src/test_runner_tests.rs", name: "check" },
    })) >= 1,
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
      command !== "pnpm install --frozen-lockfile --ignore-scripts" &&
      command !== "node harness/toolchain/pnpm.mjs",
  );
  const pin = commands.indexOf("node harness/toolchain/pnpm.mjs");
  assert.equal(
    commands.filter((command) => command === "node harness/toolchain/pnpm.mjs")
      .length,
    1,
  );
  assert.ok(pin >= 0 && pin < install);
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

const stepCommand = (prefix) =>
  checkSteps(justfile)
    .map((step) => step.command)
    .find((command) => command.startsWith(prefix));

const runIn = (directory, command) =>
  spawnSync("bash", ["-c", command], {
    cwd: directory,
    encoding: "utf8",
    env: {
      ...process.env,
      RUSTFLAGS: "-D warnings",
      RUSTC_WRAPPER: "",
      RUSTC_WORKSPACE_WRAPPER: "",
      CARGO_TARGET_DIR: join(directory, "target"),
    },
  });

test("the cargo fmt step of just check fails on a workspace with a misformatted file", () => {
  const command = stepCommand("cargo fmt");
  const directory = scratch("fmt-step-");
  try {
    ["rust-toolchain.toml", "rustfmt.toml"].forEach((file) =>
      cpSync(join(repoRoot, file), join(directory, file)),
    );
    writeFileSync(
      join(directory, "Cargo.toml"),
      '[workspace]\nmembers = ["probe"]\nresolver = "3"\n',
    );
    mkdirSync(join(directory, "probe", "src"), { recursive: true });
    writeFileSync(
      join(directory, "probe", "Cargo.toml"),
      '[package]\nname = "probe"\nversion = "0.1.0"\nedition = "2024"\n',
    );
    const source = join(directory, "probe", "src", "lib.rs");
    writeFileSync(source, "pub fn probe( )->u8{1}\n");
    const misformatted = runIn(directory, command);
    assert.notEqual(misformatted.status, 0, misformatted.stdout);
    writeFileSync(source, "pub fn probe() -> u8 {\n    1\n}\n");
    const formatted = runIn(directory, command);
    assert.equal(formatted.status, 0, formatted.stderr);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("the prettier step of just check fails on a misformatted file in its paths", () => {
  const command = stepCommand("pnpm exec prettier").replace(
    /^pnpm exec prettier /u,
    `${join(repoRoot, "node_modules", ".bin", "prettier")} `,
  );
  const directory = scratch("prettier-step-");
  try {
    [
      "package.json",
      "pnpm-workspace.yaml",
      ".prettierignore",
      ".npmrc",
    ].forEach((file) => cpSync(join(repoRoot, file), join(directory, file)));
    ["apps/desktop", "harness", ".claude/hooks"].forEach((folder) =>
      mkdirSync(join(directory, folder), { recursive: true }),
    );
    writeFileSync(join(directory, ".claude", "settings.json"), "{}\n");
    writeFileSync(join(directory, "harness", "a.mjs"), "export const a = 1;\n");
    writeFileSync(
      join(directory, ".claude", "hooks", "a.mjs"),
      "export const a = 1;\n",
    );
    const file = join(directory, "apps", "desktop", "a.mjs");
    writeFileSync(file, "export const   a = {b:1}\n");
    const misformatted = runIn(directory, command);
    assert.notEqual(misformatted.status, 0, misformatted.stdout);
    assert.match(
      misformatted.stdout + misformatted.stderr,
      /apps\/desktop\/a\.mjs/u,
    );
    writeFileSync(file, "export const a = { b: 1 };\n");
    const formatted = runIn(directory, command);
    assert.equal(formatted.status, 0, formatted.stdout + formatted.stderr);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
