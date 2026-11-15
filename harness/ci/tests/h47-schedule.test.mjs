import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

const workflow = () =>
  JSON.parse(readFileSync(".github/workflows/nightly.yml", "utf8"));
test("H47 daily and weekly schedules and manual choices are bounded and explicit", () => {
  const w = workflow();
  assert.deepEqual(w.on.workflow_dispatch.inputs.mode.options, [
    "daily",
    "full",
  ]);
  assert.equal(w.on.workflow_dispatch.inputs.mode.default, "daily");
  assert.ok(w.jobs.mutants.if.includes("0 14 * * 0"));
  assert.ok(w.jobs["mutants-daily"].if.includes("0 18 * * *"));
  assert.deepEqual(w.jobs["mutants-daily"].strategy, {
    "fail-fast": false,
    matrix: { shard: "${{ fromJSON(needs.daily-plan.outputs.shards) }}" },
  });
  assert.equal(w.jobs["mutants-daily"]["timeout-minutes"], 360);
  assert.equal(w.jobs["mutants-daily"].steps[0].with.ref, "${{ github.sha }}");
  assert.equal(w.jobs["mutants-daily"].steps[0].with["fetch-depth"], 0);
  assert.equal(w.jobs["mutants-daily"].steps.at(-1).if, "always()");
  assert.deepEqual(w.jobs["notify-mutants"].needs, [
    "mutants",
    "daily-plan",
    "mutants-daily",
  ]);
});

const git = (root, args, extra = {}) => {
  const result = spawnSync("git", args, {
    cwd: root,
    encoding: "utf8",
    env: {
      ...process.env,
      GIT_CONFIG_GLOBAL: "/dev/null",
      GIT_CONFIG_NOSYSTEM: "1",
      ...extra,
    },
  });
  assert.equal(result.status, 0, result.stderr);
  return result.stdout.trim();
};
const repository = () => {
  const root = mkdtempSync(join(tmpdir(), "h47-day-"));
  git(root, ["init", "--quiet", "--initial-branch=master"]);
  return root;
};
const commit = (root, date, value) => {
  writeFileSync(join(root, "src.txt"), value);
  git(root, ["add", "src.txt"]);
  git(
    root,
    [
      "-c",
      "user.name=fixture",
      "-c",
      "user.email=fixture@example.invalid",
      "commit",
      "--quiet",
      "-m",
      value,
    ],
    { GIT_AUTHOR_DATE: date, GIT_COMMITTER_DATE: date },
  );
  return git(root, ["rev-parse", "HEAD"]);
};
test("H47 daily range covers quiet windows initial history and shallow clones", async () => {
  const { dailyRange } = await import("../mutation-range.mjs");
  const now = new Date("2026-11-16T12:00:00Z");
  const root = repository();
  const old = commit(root, "2026-11-14T10:00:00Z", "old");
  assert.equal(dailyRange(root, now), old);
  commit(root, "2026-11-16T11:00:00Z", "recent");
  assert.equal(dailyRange(root, now), old);
  const initial = repository();
  commit(initial, "2026-11-16T09:00:00Z", "first");
  assert.equal(dailyRange(initial, now), "initial");
  const clone = join(mkdtempSync(join(tmpdir(), "h47-clone-")), "repo");
  git(root, ["clone", "--quiet", "--depth=1", `file://${root}`, clone]);
  assert.equal(git(clone, ["rev-parse", "--is-shallow-repository"]), "true");
  assert.equal(dailyRange(clone, now), old);
  assert.equal(git(clone, ["rev-parse", "--is-shallow-repository"]), "false");
  const broken = join(mkdtempSync(join(tmpdir(), "h47-broken-")), "repo");
  git(root, ["clone", "--quiet", "--depth=1", `file://${root}`, broken]);
  git(broken, ["remote", "set-url", "origin", join(root, "missing")]);
  assert.throws(() => dailyRange(broken, now), /git fetch/u);
});

test("H47 daily summary requires range evidence and preserves cancellation and completion failures", () => {
  const script = join(process.cwd(), "harness/ci/mutants-summary.mjs");
  const base = "a".repeat(40);
  const head = "b".repeat(40);
  const run = (outcome, catalog, planned, receipt, completed) => {
    const root = repository();
    const directory = join(root, "target/mutants/mutants.out");
    spawnSync("mkdir", ["-p", directory]);
    writeFileSync(join(directory, "../catalog.json"), JSON.stringify(catalog));
    if (receipt !== null)
      writeFileSync(
        join(directory, "../daily.json"),
        JSON.stringify({ shard: "0/1", ...receipt }),
      );
    writeFileSync(join(directory, "mutants.json"), JSON.stringify(planned));
    for (const name of ["missed", "timeout", "caught", "unviable"])
      writeFileSync(join(directory, `${name}.txt`), "");
    if (completed !== undefined)
      writeFileSync(
        join(directory, "outcomes.json"),
        JSON.stringify(completed),
      );
    return spawnSync(process.execPath, [script], {
      cwd: root,
      encoding: "utf8",
      env: {
        ...process.env,
        MUTATION_MODE: "daily",
        MUTATION_SHARD: "0/1",
        MUTATION_OUTCOME: outcome,
      },
    });
  };
  const identity = {
    name: "src/distinct.rs:7:3: replace +",
    file: "src/distinct.rs",
  };
  for (const origin of [base, "initial"]) {
    const empty = run("success", [], [], { base: origin, head });
    assert.equal(empty.status, 0, empty.stderr);
    assert.match(empty.stdout, /변경 없음/u);
  }
  assert.equal(run("success", [], [], null).status, 1);
  assert.equal(run("success", [], [], { base, head: "invalid" }).status, 1);
  assert.equal(run("cancelled", [], [], { base, head }).status, 1);
  assert.equal(run("failure", [], [], { base, head }).status, 1);
  assert.equal(run("success", [identity], [], { base, head }).status, 1);
  assert.equal(
    run(
      "success",
      [identity],
      [identity],
      { base, head },
      {
        total_mutants: 0,
        outcomes: [{ scenario: "Baseline", summary: "Success" }],
      },
    ).status,
    1,
  );
  assert.equal(
    run(
      "success",
      [identity],
      [identity],
      { base, head },
      {
        total_mutants: 1,
        outcomes: [{ scenario: "Baseline", summary: "Failure" }],
      },
    ).status,
    1,
  );
  assert.equal(
    run(
      "success",
      [identity],
      [identity],
      { base, head },
      {
        total_mutants: 1,
        outcomes: [{ scenario: "Baseline", summary: "Success" }],
      },
    ).status,
    0,
  );
});

test("H47 daily range follows master first parents and fails closed without HEAD", async () => {
  const { dailyRange } = await import("../mutation-range.mjs");
  const now = new Date("2026-11-16T12:00:00Z");
  const root = repository();
  assert.throws(() => dailyRange(root, now), /git rev-list/u);
  const old = commit(root, "2026-11-14T10:00:00Z", "master-old");
  git(root, ["checkout", "-qb", "side"]);
  commit(root, "2026-11-15T11:00:00Z", "side-close-to-cutoff");
  git(root, ["checkout", "-q", "master"]);
  git(
    root,
    [
      "-c",
      "user.name=fixture",
      "-c",
      "user.email=fixture@example.invalid",
      "merge",
      "--no-ff",
      "--quiet",
      "-m",
      "merge",
      "side",
    ],
    {
      GIT_AUTHOR_DATE: "2026-11-16T11:30:00Z",
      GIT_COMMITTER_DATE: "2026-11-16T11:30:00Z",
    },
  );
  assert.equal(dailyRange(root, now), old);
});

test("H47 daily cutoff includes exactly the prior 24 hours", async () => {
  const { dailyRange } = await import("../mutation-range.mjs");
  const now = new Date("2026-11-16T12:00:00Z");
  const root = repository();
  commit(root, "2026-11-14T10:00:00Z", "older");
  const before = commit(root, "2026-11-15T11:30:00Z", "before-cutoff");
  commit(root, "2026-11-15T12:30:00Z", "after-cutoff");
  commit(root, "2026-11-16T11:00:00Z", "latest");
  assert.equal(dailyRange(root, now), before);
});

test("H47 manual mutation runs require the pinned master snapshot", async () => {
  const { verifySnapshot } = await import("../mutation-range.mjs");
  const root = repository();
  const head = commit(root, "2026-11-16T09:00:00Z", "snapshot");
  assert.doesNotThrow(() => verifySnapshot(root, "refs/heads/master", head));
  assert.throws(
    () => verifySnapshot(root, "refs/heads/feature", head),
    /master/u,
  );
  assert.throws(
    () => verifySnapshot(root, "refs/heads/master", "c".repeat(40)),
    /master/u,
  );
  assert.throws(
    () => verifySnapshot(root, "refs/heads/master", undefined),
    /master/u,
  );
  for (const name of ["mutants", "mutants-daily"]) {
    const job = workflow().jobs[name];
    assert.equal(job.steps[0].with.ref, "${{ github.sha }}");
    assert.deepEqual(job.steps[2], {
      name: "master snapshot",
      run: "node harness/ci/mutation-range.mjs verify",
      env: { REF: "${{ github.ref }}", SHA: "${{ github.sha }}" },
    });
  }
});
