import { strict as assert } from "node:assert";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { installActionSpec, readTools } from "../tools.mjs";
import { readPolicy, workflowFindings } from "../workflow-policy/check.mjs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

const policy = readPolicy(repoRoot);
const tools = installActionSpec(readTools(repoRoot));
const read = (name) =>
  JSON.parse(readFileSync(join(repoRoot, ".github/workflows", name), "utf8"));
const check = (path, data) =>
  workflowFindings(
    path,
    `${JSON.stringify(data, undefined, 2)}\n`,
    policy,
    tools,
  );
const ciPath = ".github/workflows/ci.yml";
const nightlyPath = ".github/workflows/nightly.yml";

const ciConcurrency = {
  group:
    "${{ github.workflow }}-${{ github.ref }}-${{ github.ref == 'refs/heads/master' && format('{0}-{1}', github.run_id, github.run_attempt) || 'branch' }}",
  "cancel-in-progress": "${{ github.ref != 'refs/heads/master' }}",
};
const mutationSlotConcurrency = {
  group:
    "${{ github.workflow }}-mutants-slot-${{ (contains(matrix.shard, '0/') || contains(matrix.shard, '2/') || contains(matrix.shard, '4/') || contains(matrix.shard, '6/') || contains(matrix.shard, '8/')) && '0' || '1' }}",
  queue: "max",
  "cancel-in-progress": false,
};

const shardSlot = (shard) =>
  ["0/", "2/", "4/", "6/", "8/"].some((part) => shard.includes(part))
    ? "0"
    : "1";

test("일간과 주간 변이 matrix는 각각 1 또는 2개의 러너로 제한한다", () => {
  const workflow = read("nightly.yml");
  for (const name of ["mutants", "mutants-daily"]) {
    assert.equal(workflow.jobs[name].strategy["max-parallel"], 2, name);
    for (const value of [0, 1.5, 3, "2", null]) {
      const changed = structuredClone(workflow);
      changed.jobs[name].strategy["max-parallel"] = value;
      assert.ok(
        check(nightlyPath, changed).includes(`strategy:${name}`),
        `${name} ${value}`,
      );
    }
    const missing = structuredClone(workflow);
    delete missing.jobs[name].strategy["max-parallel"];
    assert.ok(
      check(nightlyPath, missing).includes(`strategy:${name}`),
      `${name} missing`,
    );
    const single = structuredClone(workflow);
    single.jobs[name].strategy["max-parallel"] = 1;
    assert.ok(
      !check(nightlyPath, single).includes(`strategy:${name}`),
      `${name} one`,
    );
  }
});

test("CI는 master의 실행과 재시도마다 독립 그룹을 쓰고 다른 브랜치만 취소한다", () => {
  const workflow = read("ci.yml");
  assert.deepEqual(workflow.concurrency, ciConcurrency);
  assert.deepEqual(check(ciPath, workflow), []);
  for (const changedConcurrency of [
    { ...ciConcurrency, group: "${{ github.workflow }}-${{ github.ref }}" },
    {
      ...ciConcurrency,
      group: "${{ github.workflow }}-${{ github.ref }}-${{ github.run_id }}",
    },
    { ...ciConcurrency, "cancel-in-progress": true },
    { ...ciConcurrency, "cancel-in-progress": false },
    undefined,
  ]) {
    const changed = structuredClone(workflow);
    if (changedConcurrency === undefined) delete changed.concurrency;
    else changed.concurrency = changedConcurrency;
    assert.notDeepEqual(
      check(ciPath, changed),
      [],
      JSON.stringify(changedConcurrency),
    );
  }
});

test("야간 변이 작업만 2슬롯 대기열을 공유하고 일간 macOS·Linux 검사는 막지 않는다", () => {
  const workflow = read("nightly.yml");
  assert.equal(workflow.concurrency, undefined);
  assert.equal(workflow.jobs.changes.concurrency, undefined);
  assert.equal(workflow.jobs.macos.concurrency, undefined);
  assert.equal(workflow.jobs.linux.concurrency, undefined);
  assert.deepEqual(workflow.jobs.mutants.concurrency, mutationSlotConcurrency);
  assert.deepEqual(
    workflow.jobs["mutants-daily"].concurrency,
    mutationSlotConcurrency,
  );
  assert.equal(
    workflow.jobs.mutants.if,
    "github.event.schedule == '0 14 * * 0' || (github.event_name == 'workflow_dispatch' && inputs.mode == 'full')",
  );
  assert.equal(
    workflow.jobs["mutants-daily"].if,
    "github.event.schedule == '0 18 * * *' || (github.event_name == 'workflow_dispatch' && inputs.mode == 'daily')",
  );
  assert.deepEqual(check(nightlyPath, workflow), []);
  const weeklySlots = { 0: 0, 1: 0 };
  for (const shard of workflow.jobs.mutants.strategy.matrix.shard) {
    const slot = shardSlot(shard);
    const index = Number(shard.split("/")[0]);
    assert.equal(slot, String(index % 2), shard);
    weeklySlots[slot] += 1;
  }
  assert.deepEqual(weeklySlots, { 0: 64, 1: 64 });
  for (let total = 1; total <= 8; total += 1) {
    const slots = new Set();
    for (let index = 0; index < total; index += 1) {
      const shard = `${index}/${total}`;
      const slot = shardSlot(shard);
      assert.equal(slot, String(index % 2), shard);
      slots.add(slot);
    }
    assert.ok(slots.size <= 2);
  }
  const withTopConcurrency = structuredClone(workflow);
  withTopConcurrency.concurrency = {
    group: "${{ github.workflow }}-runner-capacity",
    queue: "max",
    "cancel-in-progress": false,
  };
  assert.ok(
    check(nightlyPath, withTopConcurrency).includes(
      "unknown-key:top.concurrency",
    ),
  );
  const withMacosConcurrency = structuredClone(workflow);
  withMacosConcurrency.jobs.macos.concurrency = mutationSlotConcurrency;
  assert.ok(
    check(nightlyPath, withMacosConcurrency).includes(
      "unknown-key:jobs.macos.concurrency",
    ),
  );
  for (const jobName of ["mutants", "mutants-daily"]) {
    for (const changedConcurrency of [
      { ...mutationSlotConcurrency, queue: "single" },
      { ...mutationSlotConcurrency, "cancel-in-progress": true },
      { ...mutationSlotConcurrency, group: "${{ github.workflow }}-mutants" },
      undefined,
    ]) {
      const changed = structuredClone(workflow);
      if (changedConcurrency === undefined)
        delete changed.jobs[jobName].concurrency;
      else changed.jobs[jobName].concurrency = changedConcurrency;
      assert.notDeepEqual(
        check(nightlyPath, changed),
        [],
        `${jobName} ${JSON.stringify(changedConcurrency)}`,
      );
    }
  }
});
