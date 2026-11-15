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
const runSummary = (outcome, files = {}) => {
  const folder = mkdtempSync(join(tmpdir(), "h47-summary-"));
  const directory = join(folder, "target/mutants/mutants.out");
  mkdirSync(directory, { recursive: true });
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

test("H47 cargo-mutants partitions cover the exact full set without overlap", () => {
  const list = (shard) => {
    const args = ["mutants", "--list", "--json", "--no-shuffle"];
    if (shard !== undefined) args.push("--shard", shard);
    const result = spawnSync("cargo", args, {
      encoding: "utf8",
      maxBuffer: 32 * 1024 * 1024,
    });
    assert.equal(result.status, 0, result.stderr);
    return JSON.parse(result.stdout).map((entry) => entry.name);
  };
  const all = list();
  const seen = new Set();
  for (const shard of workflow().jobs.mutants.strategy.matrix.shard) {
    for (const name of list(shard)) {
      assert.ok(!seen.has(name), `overlapping mutant: ${name}`);
      seen.add(name);
    }
  }
  assert.deepEqual([...seen].sort(), all.sort());
});
