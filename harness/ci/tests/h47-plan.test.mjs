import assert from "node:assert/strict";
import { test } from "node:test";
import { shardsFor, sliceFor, runAnchor } from "../mutation-plan.mjs";
import { mkdirSync, writeFileSync, readFileSync, chmodSync } from "node:fs";
import { mkdtempSync } from "../test-temp.mjs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const catalogOf = (count) =>
  Array.from({ length: count }, (_, index) => ({
    name: `src/m${index}.rs:7:3: replace +`,
    file: `src/m${index}.rs`,
  }));
test("H47 adaptive daily shards are bounded complete nonoverlapping and budget actual review windows", () => {
  for (const [count, total] of [
    [0, 1],
    [1, 1],
    [100, 1],
    [101, 2],
    [700, 7],
    [800, 8],
    [936, 8],
    [1431, 8],
  ]) {
    const shards = shardsFor(count);
    assert.deepEqual(
      shards,
      Array.from({ length: total }, (_, i) => `${i}/${total}`),
    );
    const catalog = catalogOf(count);
    const slices = shards.map((shard) => sliceFor(catalog, shard));
    assert.deepEqual(slices.flat(), catalog);
    assert.equal(new Set(slices.flat().map((entry) => entry.name)).size, count);
    assert.ok(
      (Math.max(...slices.map((slice) => slice.length)) * 172) / 2 / 60 + 10 <
        330,
    );
  }
  for (const count of [-1, 0.5, NaN, Number.MAX_SAFE_INTEGER + 1])
    assert.throws(() => shardsFor(count));
  for (const shard of [
    "0/0",
    "1/1",
    "2/2",
    "8/8",
    "00/8",
    "0/9",
    "0/128",
    "0/8;true",
  ])
    assert.throws(() => sliceFor([], shard));
});
test("H47 scheduled anchors ignore installation queue delay and preserve manual creation time", () => {
  assert.equal(
    runAnchor("schedule", "2026-11-16T18:37:00Z").toISOString(),
    "2026-11-16T18:00:00.000Z",
  );
  assert.equal(
    runAnchor("schedule", "2026-11-17T02:00:00Z").toISOString(),
    "2026-11-16T18:00:00.000Z",
  );
  assert.equal(
    runAnchor("workflow_dispatch", "2026-11-16T18:37:00Z").toISOString(),
    "2026-11-16T18:37:00.000Z",
  );
  assert.throws(() => runAnchor("push", "2026-11-16T18:00:00Z"));
  assert.throws(() => runAnchor("schedule", "bad"));
});
const summary = (mode, outcome, catalog, planned, shard, plan) => {
  const root = mkdtempSync(join(tmpdir(), "h47-plan-summary-"));
  const folder = join(root, "target/mutants/mutants.out");
  mkdirSync(folder, { recursive: true });
  writeFileSync(join(folder, "../catalog.json"), JSON.stringify(catalog));
  writeFileSync(
    join(folder, "../daily.json"),
    JSON.stringify({ base: "a".repeat(40), head: "b".repeat(40), shard }),
  );
  if (plan !== null)
    writeFileSync(join(folder, "../plan.json"), JSON.stringify(plan));
  writeFileSync(join(folder, "mutants.json"), JSON.stringify(planned));
  for (const name of ["missed", "timeout", "caught", "unviable"])
    writeFileSync(join(folder, `${name}.txt`), "");
  if (planned.length > 0)
    writeFileSync(
      join(folder, "outcomes.json"),
      JSON.stringify({
        total_mutants: planned.length,
        outcomes: [{ scenario: "Baseline", summary: "Success" }],
      }),
    );
  return spawnSync(
    process.execPath,
    [resolve("harness/ci/mutants-summary.mjs")],
    {
      cwd: root,
      encoding: "utf8",
      env: {
        ...process.env,
        MUTATION_MODE: mode,
        MUTATION_OUTCOME: outcome,
        MUTATION_SHARD: shard,
      },
    },
  );
};
test("H47 plan summaries fail closed and daily shards validate exact planned identities", () => {
  const catalog = catalogOf(5);
  const plan = { shards: ["0/1"], anchor: "2026-11-16T18:00:00Z" };
  const ready = summary("plan", "success", catalog, [], "plan", plan);
  assert.equal(ready.status, 0);
  assert.match(ready.stdout, /선택 변이: 5/u);
  assert.match(ready.stdout, /조각 수: 1/u);
  assert.equal(summary("plan", "failure", catalog, [], "plan", plan).status, 1);
  assert.equal(
    summary("plan", "cancelled", catalog, [], "plan", plan).status,
    1,
  );
  assert.equal(summary("plan", "success", catalog, [], "plan", null).status, 1);
  assert.equal(
    summary("plan", "success", catalog, [], "plan", { ...plan, anchor: "bad" })
      .status,
    1,
  );
  assert.equal(
    summary("plan", "success", catalog, [], "plan", { ...plan, anchor: 123 })
      .status,
    1,
  );
  assert.equal(
    summary("plan", "success", catalog, [], "plan", {
      ...plan,
      shards: ["0/9"],
    }).status,
    1,
  );
  assert.equal(summary("plan", "success", [], [], "plan", plan).status, 0);
  assert.equal(
    summary("daily", "success", catalog, catalog.slice(0, 3), "0/2", null)
      .status,
    0,
  );
  assert.equal(
    summary("daily", "success", catalog, catalog.slice(3), "1/2", null).status,
    0,
  );
  assert.equal(
    summary("daily", "success", catalog, catalog.slice(0, 2), "0/2", null)
      .status,
    1,
  );
  assert.equal(
    summary(
      "daily",
      "success",
      catalog,
      [{ name: "wrong", file: "wrong" }, ...catalog.slice(1, 3)],
      "0/2",
      null,
    ).status,
    1,
  );
});
test("H47 only the daily cron runs ordinary platform checks", () => {
  const jobs = JSON.parse(
    readFileSync(".github/workflows/nightly.yml", "utf8"),
  ).jobs;
  for (const key of ["changes", "linux"])
    assert.equal(jobs[key].if, "github.event.schedule == '0 18 * * *'");
  assert.equal(
    jobs.macos.if,
    "github.event.schedule == '0 18 * * *' && needs.changes.outputs.changed == 'yes'",
  );
  assert.equal(
    jobs.notify.if,
    "failure() && github.event.schedule == '0 18 * * *'",
  );
  assert.equal(jobs["daily-plan"].permissions.actions, "read");
  assert.deepEqual(jobs["mutants-daily"].needs, ["daily-plan"]);
  assert.equal(
    jobs["mutants-daily"].steps.find((step) => step.id === "mutation")[
      "timeout-minutes"
    ],
    330,
  );
});

test("H47 planner reads run metadata once emits a bounded matrix and fails closed", () => {
  const root = mkdtempSync(join(tmpdir(), "h47-plan-cli-"));
  const bin = join(root, "bin");
  mkdirSync(bin);
  const env = {
    ...process.env,
    GIT_CONFIG_GLOBAL: "/dev/null",
    GIT_CONFIG_NOSYSTEM: "1",
  };
  const git = (args) => {
    const result = spawnSync("git", args, { cwd: root, env, encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  git(["init", "--quiet", "--initial-branch=master"]);
  const dated = {
    ...env,
    GIT_AUTHOR_DATE: "2026-11-16T10:00:00Z",
    GIT_COMMITTER_DATE: "2026-11-16T10:00:00Z",
  };
  const committed = spawnSync(
    "git",
    [
      "-c",
      "user.name=fixture",
      "-c",
      "user.email=fixture@example.invalid",
      "commit",
      "--allow-empty",
      "--quiet",
      "-m",
      "first",
    ],
    { cwd: root, env: dated, encoding: "utf8" },
  );
  assert.equal(committed.status, 0, committed.stderr);
  const catalogSource = join(root, "catalog-source.json");
  writeFileSync(catalogSource, JSON.stringify(catalogOf(936)));
  const receiptSource = join(root, "receipt-source.json");
  writeFileSync(
    receiptSource,
    JSON.stringify({
      base: "initial",
      head: git(["rev-parse", "HEAD"]),
      shard: "0/1",
    }),
  );
  const gh = join(bin, "gh");
  writeFileSync(
    gh,
    '#!/bin/sh\nprintf "%s\\n" "$*" > "$H47_API_LOG"\nprintf "%s\\n" "2026-11-16T18:37:00Z"\n',
  );
  chmodSync(gh, 0o700);
  const cargo = join(bin, "cargo");
  writeFileSync(
    cargo,
    '#!/bin/sh\nprintf "%s\\n" "$*" > "$H47_CARGO_LOG"\nmkdir -p target/mutants\ncp "$H47_CATALOG" target/mutants/catalog.json\ncp "$H47_RECEIPT" target/mutants/daily.json\n',
  );
  chmodSync(cargo, 0o700);
  const output = join(root, "output.txt");
  const execute = () =>
    spawnSync(
      process.execPath,
      [resolve("harness/ci/mutation-range.mjs"), "plan"],
      {
        cwd: root,
        encoding: "utf8",
        env: {
          ...env,
          PATH: `${bin}:${env.PATH}`,
          GITHUB_EVENT_NAME: "schedule",
          GITHUB_REPOSITORY: "fixture/project",
          GITHUB_RUN_ID: "123",
          GITHUB_OUTPUT: output,
          H47_API_LOG: join(root, "api.txt"),
          H47_CARGO_LOG: join(root, "cargo.txt"),
          H47_CATALOG: catalogSource,
          H47_RECEIPT: receiptSource,
        },
      },
    );
  const good = execute();
  assert.equal(good.status, 0, good.stderr);
  assert.match(
    readFileSync(join(root, "api.txt"), "utf8"),
    /^api repos\/fixture\/project\/actions\/runs\/123 --jq \.created_at\n$/u,
  );
  assert.match(
    readFileSync(join(root, "cargo.txt"), "utf8"),
    /mutants --daily-plan initial/u,
  );
  const emitted = Object.fromEntries(
    readFileSync(output, "utf8")
      .trim()
      .split("\n")
      .map((line) => [
        line.slice(0, line.indexOf("=")),
        line.slice(line.indexOf("=") + 1),
      ]),
  );
  assert.deepEqual(
    JSON.parse(emitted.shards),
    Array.from({ length: 8 }, (_, index) => `${index}/8`),
  );
  assert.equal(emitted.anchor, "2026-11-16T18:00:00.000Z");
  assert.equal(emitted.count, "936");
  assert.deepEqual(
    JSON.parse(readFileSync(join(root, "target/mutants/plan.json"), "utf8")),
    { shards: JSON.parse(emitted.shards), anchor: emitted.anchor },
  );
  writeFileSync(output, "");
  writeFileSync(gh, "#!/bin/sh\nexit 1\n");
  assert.equal(execute().status, 1);
  assert.equal(readFileSync(output, "utf8"), "");
  writeFileSync(gh, '#!/bin/sh\nprintf "%s\\n" "2026-11-16T18:37:00Z"\n');
  writeFileSync(cargo, "#!/bin/sh\nexit 1\n");
  assert.equal(execute().status, 1);
  assert.equal(readFileSync(output, "utf8"), "");
});

test("H47 enlarged budgets and adaptive matrices stay confined to the named daily jobs", async () => {
  const { workflowFindings, readPolicy } =
    await import("../workflow-policy/check.mjs");
  const { installActionSpec, readTools } = await import("../tools.mjs");
  const policy = readPolicy(process.cwd());
  assert.equal(policy.maxTimeout, 180);
  for (const [job, step, limit] of [
    ["mutants-daily", "mutants with sandboxed tests", 331],
    ["mutants-daily", "checkout", 330],
    ["daily-plan", "daily plan", 16],
    ["daily-plan", "checkout", 15],
    ["mutants", "mutants with sandboxed tests", 330],
  ]) {
    const w = JSON.parse(readFileSync(".github/workflows/nightly.yml", "utf8"));
    w.jobs[job].steps.find((entry) => entry.name === step)["timeout-minutes"] =
      limit;
    assert.ok(
      workflowFindings(
        ".github/workflows/nightly.yml",
        `${JSON.stringify(w, undefined, 2)}\n`,
        policy,
        installActionSpec(readTools(process.cwd())),
      ).includes(`timeout:${job}`),
    );
  }
  for (const [job, limit] of [
    ["mutants-daily", 361],
    ["changes", 181],
    ["mutants", 181],
    ["daily-plan", 181],
  ]) {
    const w = JSON.parse(readFileSync(".github/workflows/nightly.yml", "utf8"));
    w.jobs[job]["timeout-minutes"] = limit;
    assert.ok(
      workflowFindings(
        ".github/workflows/nightly.yml",
        `${JSON.stringify(w, undefined, 2)}\n`,
        policy,
        installActionSpec(readTools(process.cwd())),
      ).includes(`timeout:${job}`),
    );
  }
});
