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
const nightlyConcurrency = {
  group: "${{ github.workflow }}-runner-capacity",
  queue: "max",
  "cancel-in-progress": false,
};

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

test("야간 실행은 대기열을 공유해 일간과 주간 변이의 동시 점유를 합산 제한한다", () => {
  const workflow = read("nightly.yml");
  assert.deepEqual(workflow.concurrency, nightlyConcurrency);
  assert.equal(
    workflow.jobs.mutants.if,
    "github.event.schedule == '0 14 * * 0' || (github.event_name == 'workflow_dispatch' && inputs.mode == 'full')",
  );
  assert.equal(
    workflow.jobs["mutants-daily"].if,
    "github.event.schedule == '0 18 * * *' || (github.event_name == 'workflow_dispatch' && inputs.mode == 'daily')",
  );
  assert.deepEqual(check(nightlyPath, workflow), []);
  for (const changedConcurrency of [
    { ...nightlyConcurrency, queue: "single" },
    { ...nightlyConcurrency, "cancel-in-progress": true },
    undefined,
  ]) {
    const changed = structuredClone(workflow);
    if (changedConcurrency === undefined) delete changed.concurrency;
    else changed.concurrency = changedConcurrency;
    assert.notDeepEqual(
      check(nightlyPath, changed),
      [],
      JSON.stringify(changedConcurrency),
    );
  }
});
