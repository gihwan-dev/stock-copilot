import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { workflowFindings, readPolicy } from "../workflow-policy/check.mjs";
import { installActionSpec, readTools } from "../tools.mjs";

const root = resolve(".");
const workflow = () =>
  JSON.parse(readFileSync(".github/workflows/nightly.yml", "utf8"));
const runSummary = (
  outcome,
  files = {},
  catalog = JSON.stringify([
    { name: "src/a.rs:12:9: replace >", file: "src/a.rs" },
    { name: "src/b.rs:21:3: replace +", file: "src/b.rs" },
  ]),
) => {
  const folder = mkdtempSync(join(tmpdir(), "h47-summary-"));
  const directory = join(folder, "target/mutants/mutants.out");
  mkdirSync(directory, { recursive: true });
  if (catalog !== null)
    writeFileSync(join(directory, "../catalog.json"), catalog);
  for (const [name, text] of Object.entries(files))
    writeFileSync(join(directory, name), text);
  const summary = join(folder, "summary.md");
  const result = spawnSync(
    process.execPath,
    [resolve("harness/ci/mutants-summary.mjs")],
    {
      cwd: folder,
      env: {
        ...process.env,
        GITHUB_STEP_SUMMARY: summary,
        MUTATION_OUTCOME: outcome,
        MUTATION_SHARD: "17/128",
      },
      encoding: "utf8",
    },
  );
  assert.equal(readFileSync(summary, "utf8"), result.stdout);
  return result;
};

test("H47 cancellation and missing results remain visible and fail", () => {
  for (const outcome of ["cancelled", "failure", "skipped", "success"]) {
    const result = runSummary(outcome, {
      "missed.txt": "src/a.rs:12:9: replace > with >=\n",
      "timeout.txt": "src/b.rs:21:3: replace + with -\n",
    });
    assert.equal(result.status, 1);
    assert.match(result.stdout, /17\/128/u);
    assert.match(result.stdout, /불완전/u);
    assert.match(result.stdout, /src\/a.rs:12:9/u);
    assert.match(result.stdout, /src\/b.rs:21:3/u);
  }
});

test("H47 timeout reserve and policy accept only the mutation step budget", () => {
  assert.deepEqual(
    workflowFindings(
      ".github/workflows/nightly.yml",
      `${JSON.stringify(workflow(), undefined, 2)}\n`,
      readPolicy(root),
      installActionSpec(readTools(root)),
    ),
    [],
  );
  const w = workflow();
  const job = w.jobs.mutants;
  const step = job.steps.find((entry) => entry.id === "mutation");
  assert.equal(step["timeout-minutes"], 160);
  assert.equal(job["timeout-minutes"], 180);
  assert.equal(job.steps.at(-1).if, "always()");
  assert.equal(
    job.steps.at(-1).env.MUTATION_OUTCOME,
    "${{ steps.mutation.outcome }}",
  );
  assert.deepEqual(
    job.strategy.matrix.shard,
    Array.from({ length: 128 }, (_, i) => `${i}/128`),
  );
  for (const [name, timeout] of [
    ["mutants without network", 181],
    ["checkout", 160],
  ]) {
    const changed = workflow();
    changed.jobs.mutants.steps.find((entry) => entry.name === name)[
      "timeout-minutes"
    ] = timeout;
    assert.ok(
      workflowFindings(
        ".github/workflows/nightly.yml",
        `${JSON.stringify(changed, undefined, 2)}\n`,
        readPolicy(root),
        installActionSpec(readTools(root)),
      ).includes("timeout:mutants"),
    );
  }
});

test("H47 completion requires a successful baseline and every planned mutant", () => {
  const files = {
    "mutants.json": JSON.stringify([{}, {}]),
    "outcomes.json": JSON.stringify({
      total_mutants: 2,
      outcomes: [{ scenario: "Baseline", summary: "Success" }],
    }),
  };
  assert.equal(
    runSummary("success", {
      "mutants.json": "[]",
      "outcomes.json": JSON.stringify({
        total_mutants: 0,
        outcomes: [{ scenario: "Baseline", summary: "Success" }],
      }),
    }).status,
    1,
  );
  const complete = runSummary("success", files);
  assert.equal(complete.status, 0);
  assert.match(complete.stdout, /완주/u);
  for (const outcomes of [
    {
      total_mutants: 1,
      outcomes: [{ scenario: "Baseline", summary: "Success" }],
    },
    {
      total_mutants: 3,
      outcomes: [{ scenario: "Baseline", summary: "Success" }],
    },
    {
      total_mutants: 2,
      outcomes: [{ scenario: "Baseline", summary: "Failure" }],
    },
    { total_mutants: 2, outcomes: [] },
  ])
    assert.equal(
      runSummary("success", {
        ...files,
        "outcomes.json": JSON.stringify(outcomes),
      }).status,
      1,
    );
  assert.equal(runSummary("cancelled", files).status, 1);
  assert.equal(
    runSummary("success", { ...files, "mutants.json": "[]" }).status,
    1,
  );
  assert.equal(
    runSummary("success", { ...files, "outcomes.json": "{" }).status,
    1,
  );
});

test("H47 successful empty slices have an explicit empty result without baseline", () => {
  const files = {
    "mutants.json": "[]",
    "missed.txt": "",
    "timeout.txt": "",
    "caught.txt": "",
    "unviable.txt": "",
  };
  const empty = runSummary("success", files);
  assert.equal(empty.status, 0);
  assert.match(empty.stdout, /대상 없음/u);
  for (const outcome of ["cancelled", "failure", "skipped", ""])
    assert.equal(runSummary(outcome, files).status, 1);
  for (const name of [
    "mutants.json",
    "missed.txt",
    "timeout.txt",
    "caught.txt",
    "unviable.txt",
  ]) {
    const missing = { ...files };
    delete missing[name];
    assert.equal(runSummary("success", missing).status, 1);
  }
  for (const name of [
    "missed.txt",
    "timeout.txt",
    "caught.txt",
    "unviable.txt",
  ])
    assert.equal(
      runSummary("success", { ...files, [name]: "unexpected result" }).status,
      1,
    );
  assert.equal(
    runSummary("success", { ...files, "outcomes.json": "{" }).status,
    1,
  );
});

test("H47 cargo-mutants partitions cover the exact full set without overlap", () => {
  const list = (shard) => {
    const args =
      shard === undefined
        ? ["mutants", "--list", "--json", "--no-shuffle"]
        : [
            "mutants",
            "--no-config",
            "--cap-lints=true",
            "--test-tool=nextest",
            "--list",
            "--json",
            "--no-shuffle",
            "--file",
            "crates/domain/src/**/*.rs",
            "--file",
            "crates/order-execution/src/**/*.rs",
            "--file",
            "crates/application/src/**/*.rs",
          ];
    if (shard !== undefined) args.push("--shard", shard);
    const result = spawnSync("cargo", args, {
      encoding: "utf8",
      maxBuffer: 32 * 1024 * 1024,
    });
    assert.equal(result.status, 0, result.stderr);
    return JSON.parse(result.stdout).map((entry) => entry.name);
  };
  const all = list();
  assert.ok(
    all.length > 0,
    "the configured full mutation catalog must not be empty",
  );
  const seen = new Set();
  for (const shard of workflow().jobs.mutants.strategy.matrix.shard) {
    for (const name of list(shard)) {
      assert.ok(!seen.has(name), `overlapping mutant: ${name}`);
      seen.add(name);
    }
  }
  assert.deepEqual([...seen].sort(), all.sort());
});

test("H47 actual empty slice artifacts are successful without outcomes", () => {
  const folder = mkdtempSync(join(tmpdir(), "h47-empty-"));
  mkdirSync(join(folder, "src"));
  writeFileSync(
    join(folder, "Cargo.toml"),
    '[package]\nname="h47-empty"\nversion="0.1.0"\nedition="2024"\n[workspace]\n',
  );
  writeFileSync(
    join(folder, "src/lib.rs"),
    "pub fn positive(value: i32) -> bool { value > 0 }\n",
  );
  const locked = spawnSync("cargo", ["generate-lockfile", "--offline"], {
    cwd: folder,
    encoding: "utf8",
  });
  assert.equal(locked.status, 0, locked.stderr);
  mkdirSync(join(folder, "target/mutants"), { recursive: true });
  const result = spawnSync(
    "cargo",
    [
      "mutants",
      "--no-config",
      "--cap-lints=true",
      "--test-tool=nextest",
      "--no-shuffle",
      "--shard",
      "127/128",
      "--output",
      "target/mutants",
    ],
    { cwd: folder, encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
  const catalog = spawnSync(
    "cargo",
    ["mutants", "--no-config", "--list", "--json"],
    { cwd: folder, encoding: "utf8" },
  );
  assert.equal(catalog.status, 0, catalog.stderr);
  assert.ok(JSON.parse(catalog.stdout).length > 0);
  writeFileSync(join(folder, "target/mutants/catalog.json"), catalog.stdout);
  const directory = join(folder, "target/mutants/mutants.out");
  assert.deepEqual(
    JSON.parse(readFileSync(join(directory, "mutants.json"), "utf8")),
    [],
  );
  const summary = join(folder, "summary.md");
  const report = spawnSync(
    process.execPath,
    [resolve("harness/ci/mutants-summary.mjs")],
    {
      cwd: folder,
      env: {
        ...process.env,
        MUTATION_OUTCOME: "success",
        MUTATION_SHARD: "127/128",
        GITHUB_STEP_SUMMARY: summary,
      },
      encoding: "utf8",
    },
  );
  assert.equal(report.status, 0, report.stderr);
  assert.match(report.stdout, /대상 없음/u);
  assert.equal(readFileSync(summary, "utf8"), report.stdout);
});

test("H47 missing empty or malformed global catalogs never validate an empty shard", () => {
  const files = {
    "mutants.json": "[]",
    "missed.txt": "",
    "timeout.txt": "",
    "caught.txt": "",
    "unviable.txt": "",
  };
  for (const catalog of [
    null,
    "[]",
    "{}",
    "{",
    "[null]",
    '[{"name":"good","file":"src/good.rs"},null]',
    '[{"name":"n"}]',
    '[{"file":"src/a.rs"}]',
    '[{"name":"", "file":"src/a.rs"}]',
  ]) {
    const result = runSummary("success", files, catalog);
    assert.equal(result.status, 1);
    assert.match(result.stdout, /불완전/u);
  }
});
