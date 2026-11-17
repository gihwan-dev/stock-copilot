import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { FRESH_STEPS } from "../../../.claude/hooks/lib/check-sandbox.mjs";
import { dumpJustfile, renderLine } from "../../check-list/list.mjs";
import { knownGaps } from "../../known-gaps/list.mjs";
import { installActionSpec, readTools } from "../tools.mjs";
import {
  readPolicy,
  repositoryFindings,
  workflowFindings,
} from "../workflow-policy/check.mjs";
import { childEnvironment, repoRoot, scratch } from "./signature-repo.mjs";

const policy = readPolicy(repoRoot);
const toolSpec = installActionSpec(readTools(repoRoot));
const workflows = [
  [".github/workflows/ci.yml", "linux", "ubuntu-24.04-arm"],
  [".github/workflows/ci.yml", "macos", "macos-15"],
  [".github/workflows/nightly.yml", "linux", "ubuntu-24.04-arm"],
  [".github/workflows/nightly.yml", "macos", "macos-15"],
];

const readWorkflow = (path) =>
  JSON.parse(readFileSync(join(repoRoot, path), "utf8"));

const findings = (path, workflow) =>
  workflowFindings(
    path,
    `${JSON.stringify(workflow, undefined, 2)}\n`,
    policy,
    toolSpec,
  );

test("remote checks run on hosted runners without restoring a compiled target", () => {
  assert.deepEqual(repositoryFindings(repoRoot, policy, toolSpec), []);
  for (const [path, name, runner] of workflows) {
    const workflow = readWorkflow(path);
    const job = workflow.jobs[name];
    assert.equal(job["runs-on"], runner, `${path} ${name}`);
    assert.ok(
      job.steps.some((step) => step.uses?.startsWith("actions/checkout@")),
    );
    assert.ok(
      job.steps.some((step) =>
        /^just (?:base=.* )?(?:check|linux-check)$/u.test(step.run ?? ""),
      ),
    );
    assert.doesNotMatch(
      JSON.stringify(job),
      /seed-target|sccache|actions\/cache|rust-cache|CARGO_TARGET_DIR|RUSTC_WRAPPER/u,
    );
  }
});

test("workflow policy rejects compiled cache restoration, seed commands, and self-hosted runners", () => {
  for (const [path, name] of workflows) {
    const restored = readWorkflow(path);
    restored.jobs[name].steps.push({
      name: "restore compiled target",
      uses: `actions/cache@${"a".repeat(40)}`,
    });
    assert.ok(
      findings(path, restored).some((rule) =>
        rule.startsWith("unlisted-action:"),
      ),
      `${path} ${name} cache`,
    );
    const seeded = readWorkflow(path);
    seeded.jobs[name].steps.push({ name: "seed", run: "just seed-target" });
    assert.ok(
      findings(path, seeded).includes(`command:${name}`),
      `${path} ${name} seed`,
    );
    const selfHosted = readWorkflow(path);
    selfHosted.jobs[name]["runs-on"] = "self-hosted";
    assert.ok(
      findings(path, selfHosted).includes(`runner:${name}`),
      `${path} ${name} runner`,
    );
  }
});

test("fresh and remote check entrypoints do not request a build seed", () => {
  const recipes = dumpJustfile(join(repoRoot, "justfile")).recipes;
  for (const name of [
    "fresh-build",
    "fresh-cargo",
    "fresh-all",
    "fresh-node",
    "check",
    "check-ts",
    "check-fast",
    "linux-check",
    "linux-test",
    "coverage-check",
    "mutants-shard",
    "mutants-daily",
  ]) {
    assert.ok(recipes[name], name);
    assert.doesNotMatch(
      JSON.stringify(recipes[name]),
      /seed-target|seed\.py|sccache/u,
      name,
    );
  }
});

test("the known dependency-artifact gap is listed and full cleanup is manual", () => {
  const gaps = knownGaps(repoRoot);
  assert.ok(
    gaps.some(
      (gap) =>
        gap.location.startsWith("docs/adr/h-51-shared-compile-cache.md:") &&
        gap.text.includes("외부 의존성 산출물") &&
        gap.text.startsWith(
          "`fresh-cargo`와 종료 훅의 `cargo clean --workspace` 뒤에도 외부 의존성 산출물이 남는다.",
        ),
    ),
  );
  const recipes = dumpJustfile(join(repoRoot, "justfile")).recipes;
  assert.deepEqual(recipes["fresh-all"].body.map(renderLine), [
    "cargo clean --locked",
    "cargo llvm-cov clean --locked",
  ]);
  assert.deepEqual(
    recipes["fresh-build"].dependencies.map((dependency) => dependency.recipe),
    ["fresh-cargo", "fresh-node"],
  );
  assert.doesNotMatch(
    readFileSync(join(repoRoot, "lefthook.yml"), "utf8"),
    /fresh-all/u,
  );
  assert.doesNotMatch(
    readFileSync(join(repoRoot, ".claude/hooks/lib/check-sandbox.mjs"), "utf8"),
    /fresh-all/u,
  );
  assert.deepEqual(FRESH_STEPS, [
    ["cargo", "clean", "--locked", "--offline", "--workspace"],
    ["cargo", "llvm-cov", "clean", "--locked", "--offline", "--workspace"],
    [
      "pnpm",
      "install",
      "--frozen-lockfile",
      "--force",
      "--offline",
      "--ignore-scripts",
    ],
  ]);
});

test("workspace cleanup preserves external dependency artifacts and manual full cleanup removes them", () => {
  const directory = scratch("h51-clean-");
  const workspace = join(directory, "workspace");
  const dependency = join(directory, "dependency");
  try {
    mkdirSync(join(workspace, "src"), { recursive: true });
    mkdirSync(join(dependency, "src"), { recursive: true });
    writeFileSync(
      join(workspace, "rust-toolchain.toml"),
      readFileSync(join(repoRoot, "rust-toolchain.toml")),
    );
    writeFileSync(
      join(workspace, "Cargo.toml"),
      '[package]\nname="h51-workspace"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nh51-dependency={path="../dependency"}\n[workspace]\n',
    );
    writeFileSync(
      join(workspace, "src/lib.rs"),
      "pub fn value() -> u32 { h51_dependency::value() }\n",
    );
    writeFileSync(
      join(dependency, "Cargo.toml"),
      '[package]\nname="h51-dependency"\nversion="0.1.0"\nedition="2024"\n[workspace]\n',
    );
    writeFileSync(
      join(dependency, "src/lib.rs"),
      "pub fn value() -> u32 { 7 }\n",
    );
    const env = childEnvironment({
      CARGO_BUILD_JOBS: "4",
      NEXTEST_TEST_THREADS: "4",
      CARGO_HOME: join(directory, "cargo-home"),
    });
    const run = (command, args) => {
      const result = spawnSync(command, args, {
        cwd: workspace,
        env,
        encoding: "utf8",
      });
      assert.equal(
        result.status,
        0,
        `${command}: ${result.stdout}\n${result.stderr}`,
      );
    };
    run("cargo", ["generate-lockfile", "--offline"]);
    run("cargo", ["build", "--locked", "--offline"]);
    const deps = join(workspace, "target/debug/deps");
    const dependencyExists = () =>
      existsSync(deps) &&
      readdirSync(deps).some((name) =>
        /^libh51_dependency-.*\.rlib$/u.test(name),
      );
    assert.equal(dependencyExists(), true);
    const just = (recipe) =>
      run("just", [
        "--justfile",
        join(repoRoot, "justfile"),
        "--working-directory",
        workspace,
        recipe,
      ]);
    just("fresh-cargo");
    assert.equal(dependencyExists(), true);
    just("fresh-all");
    assert.equal(dependencyExists(), false);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});
