import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { workflowFindings, readPolicy } from "../workflow-policy/check.mjs";
import { installActionSpec, readTools } from "../tools.mjs";

const root = new URL("../../../", import.meta.url).pathname;
const path = ".github/workflows/nightly.yml";
const workflow = () =>
  JSON.parse(
    readFileSync(
      new URL("../../../.github/workflows/nightly.yml", import.meta.url),
      "utf8",
    ),
  );

test("H-42 mutation shards run independently and notify with their own title", () => {
  const jobs = workflow().jobs;
  assert.equal(jobs.mutants.needs, undefined);
  assert.equal(jobs.mutants.if, undefined);
  assert.equal(jobs.mutants["timeout-minutes"], 120);
  assert.deepEqual(jobs.mutants.strategy, {
    "fail-fast": false,
    matrix: { shard: ["0/4", "1/4", "2/4", "3/4"] },
  });
  assert.deepEqual(jobs["notify-mutants"].needs, ["mutants"]);
  assert.equal(jobs["notify-mutants"].if, "failure()");
  assert.equal(jobs["notify-mutants"].steps[2].env.TITLE, "밤 변이 검사 실패");
  assert.ok(
    jobs.mutants.steps.some(
      (step) =>
        step.run ===
        "node harness/ci/linux/offline.mjs just mutants-shard ${{ matrix.shard }}",
    ),
  );
  assert.ok(!jobs.linux.steps.some((step) => /mutants/u.test(step.run ?? "")));
});

test("H-42 policy rejects unsafe shard strategies", () => {
  for (const strategy of [
    undefined,
    { "fail-fast": true, matrix: { shard: ["1/4"] } },
    { matrix: { shard: ["$(evil)"] } },
  ]) {
    const changed = workflow();
    changed.jobs.mutants.strategy = strategy;
    assert.ok(
      workflowFindings(
        path,
        `${JSON.stringify(changed, undefined, 2)}\n`,
        readPolicy(root),
        installActionSpec(readTools(root)),
      ).includes("strategy:mutants"),
    );
  }
});
