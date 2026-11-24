import { strict as assert } from "node:assert";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  MAX_SHARDS,
  MUTANTS_PER_SHARD,
  buildPlan,
  packageOfSource,
  partitionCatalog,
  planShards,
  runPlan,
} from "../diff-mutants-plan.mjs";
import { summarizeShard, verifyDiffShard } from "../diff-mutants-shard.mjs";
import {
  evaluateAggregate,
  expectedShardJobs,
  parseJobPages,
} from "../diff-mutants-aggregate.mjs";
import { checkRun, failureCode } from "../remote-check.mjs";
import { mkdtempSync } from "../test-temp.mjs";
import { installActionSpec, readTools } from "../tools.mjs";
import { readPolicy, workflowFindings } from "../workflow-policy/check.mjs";
import { repoRoot } from "./signature-repo.mjs";

const BASE = "a".repeat(40);
const HEAD = "b".repeat(40);
const OTHER = "c".repeat(40);
const EXCLUSIONS = JSON.parse(
  readFileSync(join(repoRoot, "harness/ci/linux/exclusions.json"), "utf8"),
);
const POLICY = readPolicy(repoRoot);
const TOOL_SPEC = installActionSpec(readTools(repoRoot));
const CI_PATH = ".github/workflows/ci.yml";

const makeMutants = (file, count, prefix = "m") =>
  Array.from({ length: count }, (_, index) => ({
    name: `${file}:${index + 1}:1: replace ${prefix}_${index}`,
    file,
  }));

const makeEvidence = (overrides = {}) => ({
  "missed.txt": "",
  "timeout.txt": "",
  "caught.txt": "caught\n",
  "unviable.txt": "",
  ...overrides,
});

const makeJob = (name, steps, overrides = {}) => ({
  name,
  head_sha: HEAD,
  run_attempt: 1,
  status: "completed",
  conclusion: "success",
  steps: steps.map((step) => ({
    name: step,
    status: "completed",
    conclusion: "success",
  })),
  ...overrides,
});

test("H-63 계획은 플랫폼 중립 크레이트를 Linux로, 제외 크레이트를 macOS로 나누고 조각 상한 8을 지킨다", () => {
  assert.equal(MUTANTS_PER_SHARD, 20);
  assert.equal(MAX_SHARDS, 8);
  assert.deepEqual(planShards(0), []);
  assert.deepEqual(planShards(1), ["0/1"]);
  assert.deepEqual(planShards(20), ["0/1"]);
  assert.deepEqual(planShards(21), ["0/2", "1/2"]);
  assert.deepEqual(planShards(130), [
    "0/7",
    "1/7",
    "2/7",
    "3/7",
    "4/7",
    "5/7",
    "6/7",
  ]);
  assert.deepEqual(planShards(500), [
    "0/8",
    "1/8",
    "2/8",
    "3/8",
    "4/8",
    "5/8",
    "6/8",
    "7/8",
  ]);
  for (const invalid of [-1, 1.5, NaN, "10"]) {
    assert.throws(() => planShards(invalid));
  }
  assert.equal(packageOfSource("crates/domain/src/lib.rs"), "domain");
  assert.equal(packageOfSource("crates/application/src/lib.rs"), "application");
  assert.equal(
    packageOfSource("crates/order-execution/src/lib.rs"),
    "order-execution",
  );
  assert.equal(
    packageOfSource("crates/adapters/store-sqlite/src/lib.rs"),
    "store-sqlite",
  );
  assert.throws(() => packageOfSource("crates/unknown/src/lib.rs"));
  assert.throws(() => packageOfSource("crates/domain/tests/lib.rs"));

  const catalog = [
    ...makeMutants("crates/domain/src/lib.rs", 100, "dom"),
    ...makeMutants("crates/application/src/lib.rs", 20, "app"),
    ...makeMutants("crates/order-execution/src/lib.rs", 10, "ord"),
    ...makeMutants("crates/adapters/store-sqlite/src/lib.rs", 25, "sql"),
  ];
  const partitioned = partitionCatalog(catalog, EXCLUSIONS);
  assert.equal(partitioned.linux.length, 130);
  assert.equal(partitioned.macos.length, 25);

  const plan = buildPlan(BASE, catalog, EXCLUSIONS);
  assert.deepEqual(plan, {
    base: BASE,
    total: 155,
    linuxCount: 130,
    macosCount: 25,
    linuxShards: ["0/7", "1/7", "2/7", "3/7", "4/7", "5/7", "6/7"],
    macosShards: ["0/2", "1/2"],
    runLinux: "yes",
    runMacos: "yes",
  });

  const emptyPlan = buildPlan(BASE, [], EXCLUSIONS);
  assert.deepEqual(emptyPlan, {
    base: BASE,
    total: 0,
    linuxCount: 0,
    macosCount: 0,
    linuxShards: [],
    macosShards: [],
    runLinux: "no",
    runMacos: "no",
  });

  assert.throws(() => buildPlan("not-a-sha", catalog, EXCLUSIONS));
  assert.throws(() =>
    buildPlan(
      BASE,
      [{ name: "x", file: "crates/mystery/src/lib.rs" }],
      EXCLUSIONS,
    ),
  );
});

test("H-63 계획 실행기는 실패한 사전 검사에서 출력을 기록하지 않고 닫힌 실패로 끝난다", () => {
  const emitted = [];
  const written = [];
  assert.throws(() =>
    runPlan(repoRoot, BASE, {
      run: () => 1,
      readJson: () => [],
      writeJson: (path, data) => written.push([path, data]),
      emit: (outputs) => emitted.push(outputs),
      summarize: () => {},
    }),
  );
  assert.equal(emitted.length, 0);
  assert.equal(written.length, 0);

  const catalog = makeMutants("crates/domain/src/lib.rs", 41);
  const plan = runPlan(repoRoot, BASE, {
    run: () => 0,
    readJson: (path) => (path.endsWith("catalog.json") ? catalog : EXCLUSIONS),
    writeJson: (path, data) => written.push([path, data]),
    emit: (outputs) => emitted.push(outputs),
    summarize: () => {},
  });
  assert.equal(plan.linuxShards.length, 3);
  assert.equal(plan.macosShards.length, 0);
  assert.equal(emitted.length, 1);
  assert.equal(emitted[0].run_linux, "yes");
  assert.equal(emitted[0].run_macos, "no");
  assert.equal(emitted[0].linux_shards, '["0/3","1/3","2/3"]');
  assert.equal(emitted[0].macos_shards, "[]");
});

test("H-63 조각 요약은 영수증·목록·완주·시간 초과·놓침 증거를 검증하고 불완전하면 실패한다", () => {
  const catalog = makeMutants("crates/domain/src/lib.rs", 5);
  const slice0 = catalog.slice(0, 3);
  const receipt = { base: BASE, head: HEAD, platform: "linux", shard: "0/2" };
  const outcomes = {
    total_mutants: 3,
    outcomes: [{ scenario: "Baseline", summary: "Success" }],
  };
  const expected = {
    base: BASE,
    head: HEAD,
    platform: "linux",
    shard: "0/2",
    expectedCount: 5,
  };
  assert.equal(
    verifyDiffShard(
      catalog,
      receipt,
      slice0,
      outcomes,
      makeEvidence(),
      expected,
      EXCLUSIONS,
    ),
    "완주",
  );
  assert.throws(() =>
    verifyDiffShard(
      catalog,
      { ...receipt, platform: "macos" },
      slice0,
      outcomes,
      makeEvidence(),
      expected,
      EXCLUSIONS,
    ),
  );
  assert.throws(() =>
    verifyDiffShard(
      catalog,
      { ...receipt, head: OTHER },
      slice0,
      outcomes,
      makeEvidence(),
      expected,
      EXCLUSIONS,
    ),
  );
  assert.throws(() =>
    verifyDiffShard(
      catalog.slice(0, 4),
      receipt,
      slice0,
      outcomes,
      makeEvidence(),
      expected,
      EXCLUSIONS,
    ),
  );
  assert.throws(() =>
    verifyDiffShard(
      [
        ...catalog.slice(0, 4),
        makeMutants("crates/adapters/store-sqlite/src/lib.rs", 1)[0],
      ],
      receipt,
      slice0,
      outcomes,
      makeEvidence(),
      expected,
      EXCLUSIONS,
    ),
  );
  assert.throws(() =>
    verifyDiffShard(
      catalog,
      receipt,
      catalog.slice(1, 4),
      outcomes,
      makeEvidence(),
      expected,
      EXCLUSIONS,
    ),
  );
  assert.throws(() =>
    verifyDiffShard(
      catalog,
      receipt,
      slice0,
      { ...outcomes, total_mutants: 2 },
      makeEvidence(),
      expected,
      EXCLUSIONS,
    ),
  );
  assert.throws(() =>
    verifyDiffShard(
      catalog,
      receipt,
      slice0,
      outcomes,
      makeEvidence({
        "timeout.txt": "crates/domain/src/lib.rs:1:1: timeout\n",
      }),
      expected,
      EXCLUSIONS,
    ),
  );
  assert.throws(() =>
    verifyDiffShard(
      catalog,
      receipt,
      slice0,
      outcomes,
      makeEvidence({ "missed.txt": undefined }),
      expected,
      EXCLUSIONS,
    ),
  );
});

test("H-63 조각 요약은 실패·시간 초과·취소 시에도 missed.txt와 timeout.txt 증거를 요약에 남기고 0이 아닌 판정을 낸다", () => {
  const dir = mkdtempSync(join(repoRoot, "target/h63-shard-"));
  try {
    const out = join(dir, "target/mutants/mutants.out");
    mkdirSync(join(dir, "harness/ci/linux"), { recursive: true });
    mkdirSync(out, { recursive: true });
    writeFileSync(
      join(dir, "harness/ci/linux/exclusions.json"),
      JSON.stringify(EXCLUSIONS),
    );
    const catalog = makeMutants("crates/domain/src/lib.rs", 2);
    writeFileSync(
      join(dir, "target/mutants/catalog.json"),
      JSON.stringify(catalog),
    );
    writeFileSync(
      join(dir, "target/mutants/diff-shard.json"),
      JSON.stringify({
        base: BASE,
        head: HEAD,
        platform: "linux",
        shard: "0/1",
      }),
    );
    writeFileSync(join(out, "mutants.json"), JSON.stringify(catalog));
    writeFileSync(
      join(out, "outcomes.json"),
      JSON.stringify({
        total_mutants: 2,
        outcomes: [{ scenario: "Baseline", summary: "Success" }],
      }),
    );
    writeFileSync(
      join(out, "missed.txt"),
      "crates/domain/src/lib.rs:1:1: replace > with >=\n",
    );
    writeFileSync(
      join(out, "timeout.txt"),
      "crates/domain/src/lib.rs:2:1: replace + with -\n",
    );
    writeFileSync(join(out, "caught.txt"), "");
    writeFileSync(join(out, "unviable.txt"), "");
    const failed = summarizeShard(dir, {
      MUTATION_OUTCOME: "failure",
      MUTATION_BASE: BASE,
      MUTATION_SHARD: "0/1",
      MUTATION_PLATFORM: "linux",
      MUTATION_EXPECTED_COUNT: "2",
      SHA: HEAD,
    });
    assert.equal(failed.ok, false);
    assert.match(failed.summary, /replace > with >=/u);
    assert.match(failed.summary, /replace \+ with -/u);
    const timedOut = summarizeShard(dir, {
      MUTATION_OUTCOME: "timed_out",
      MUTATION_BASE: BASE,
      MUTATION_SHARD: "0/1",
      MUTATION_PLATFORM: "linux",
      MUTATION_EXPECTED_COUNT: "2",
      SHA: HEAD,
    });
    assert.equal(timedOut.ok, false);
    assert.match(timedOut.summary, /replace \+ with -/u);
    writeFileSync(join(out, "missed.txt"), "");
    writeFileSync(join(out, "timeout.txt"), "");
    const passed = summarizeShard(dir, {
      MUTATION_OUTCOME: "success",
      MUTATION_BASE: BASE,
      MUTATION_SHARD: "0/1",
      MUTATION_PLATFORM: "linux",
      MUTATION_EXPECTED_COUNT: "2",
      SHA: HEAD,
    });
    assert.equal(passed.ok, true);
    assert.equal(passed.state, "완주");
    const missingOutcome = summarizeShard(dir, {
      MUTATION_BASE: BASE,
      MUTATION_SHARD: "0/1",
      MUTATION_PLATFORM: "linux",
      MUTATION_EXPECTED_COUNT: "2",
      SHA: HEAD,
    });
    assert.equal(missingOutcome.ok, false);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("H-63 최종 합산은 조각 누락·실패·시간 초과·취소·건너뜀·중복·비일치 중 하나라도 있으면 실패한다", () => {
  const input = {
    sha: HEAD,
    runAttempt: "1",
    planResult: "success",
    linuxResult: "success",
    macosResult: "success",
    total: "45",
    linuxCount: "35",
    macosCount: "10",
    linuxShards: '["0/2","1/2"]',
    macosShards: '["0/1"]',
    runLinux: "yes",
    runMacos: "yes",
  };
  const planJob = makeJob("diff-mutants-plan", ["range", "diff mutants plan"]);
  const shardNames = expectedShardJobs(["0/2", "1/2"], ["0/1"]);
  const shardJobs = shardNames.map((name) =>
    makeJob(name, ["diff mutants shard", "diff mutants summary"]),
  );
  const allJobs = [planJob, ...shardJobs];
  assert.equal(evaluateAggregate(input, allJobs).ok, true);

  assert.equal(
    evaluateAggregate(
      {
        ...input,
        total: "0",
        linuxCount: "0",
        macosCount: "0",
        linuxShards: "[]",
        macosShards: "[]",
        runLinux: "no",
        runMacos: "no",
        linuxResult: "skipped",
        macosResult: "skipped",
      },
      [planJob],
    ).ok,
    true,
  );

  assert.equal(
    evaluateAggregate(input, [planJob, shardJobs[0], shardJobs[2]]).ok,
    false,
  );
  assert.equal(evaluateAggregate(input, [...allJobs, shardJobs[0]]).ok, false);
  assert.equal(
    evaluateAggregate(input, [
      ...allJobs,
      makeJob("diff-mutants-macos (1/2)", [
        "diff mutants shard",
        "diff mutants summary",
      ]),
    ]).ok,
    false,
  );
  for (const badConclusion of [
    "failure",
    "timed_out",
    "cancelled",
    "skipped",
  ]) {
    assert.equal(
      evaluateAggregate(input, [
        planJob,
        { ...shardJobs[0], conclusion: badConclusion },
        shardJobs[1],
        shardJobs[2],
      ]).ok,
      false,
      badConclusion,
    );
  }
  assert.equal(
    evaluateAggregate(input, [
      planJob,
      {
        ...shardJobs[0],
        steps: [
          {
            name: "diff mutants shard",
            status: "completed",
            conclusion: "success",
          },
        ],
      },
      shardJobs[1],
      shardJobs[2],
    ]).ok,
    false,
  );
  assert.equal(
    evaluateAggregate(input, [
      planJob,
      { ...shardJobs[0], head_sha: OTHER },
      shardJobs[1],
      shardJobs[2],
    ]).ok,
    false,
  );
  assert.equal(
    evaluateAggregate(input, [
      planJob,
      { ...shardJobs[0], run_attempt: 2 },
      shardJobs[1],
      shardJobs[2],
    ]).ok,
    false,
  );
  assert.equal(
    evaluateAggregate({ ...input, planResult: "failure" }, allJobs).ok,
    false,
  );
  assert.equal(
    evaluateAggregate({ ...input, linuxResult: "skipped" }, allJobs).ok,
    false,
  );
  assert.equal(
    evaluateAggregate({ ...input, linuxShards: '["0/1"]' }, allJobs).ok,
    false,
  );
  assert.equal(evaluateAggregate({ ...input, total: "44" }, allJobs).ok, false);

  assert.deepEqual(
    parseJobPages([
      { total_count: 2, jobs: [planJob] },
      { total_count: 2, jobs: [shardJobs[0]] },
    ]).length,
    2,
  );
  assert.throws(() => parseJobPages([{ total_count: 3, jobs: [planJob] }]));
  assert.throws(() =>
    parseJobPages([
      { total_count: 2, jobs: [planJob] },
      { total_count: 3, jobs: [shardJobs[0]] },
    ]),
  );
});

test("H-63 워크플로 정책은 macOS 동시 실행 상한 2·2슬롯 대기열과 Linux 상한 4를 강제한다", () => {
  const ci = JSON.parse(readFileSync(join(repoRoot, CI_PATH), "utf8"));
  const check = (candidate) =>
    workflowFindings(
      CI_PATH,
      `${JSON.stringify(candidate, undefined, 2)}\n`,
      POLICY,
      TOOL_SPEC,
    );
  assert.deepEqual(check(ci), []);
  assert.equal(ci.jobs["diff-mutants-macos"]["runs-on"], "macos-15");
  assert.equal(ci.jobs["diff-mutants-macos"].strategy["max-parallel"], 2);
  assert.equal(ci.jobs["diff-mutants-linux"]["runs-on"], "ubuntu-24.04-arm");
  assert.equal(ci.jobs["diff-mutants-linux"].strategy["max-parallel"], 4);
  assert.ok(ci.jobs.notify.needs.includes("diff-mutants-verdict"));

  const tooManyMacos = structuredClone(ci);
  tooManyMacos.jobs["diff-mutants-macos"].strategy["max-parallel"] = 3;
  assert.ok(check(tooManyMacos).includes("strategy:diff-mutants-macos"));

  const noMacosConcurrency = structuredClone(ci);
  delete noMacosConcurrency.jobs["diff-mutants-macos"].concurrency;
  assert.ok(
    check(noMacosConcurrency).includes(
      "missing-key:jobs.diff-mutants-macos.concurrency",
    ),
  );

  const tooManyLinux = structuredClone(ci);
  tooManyLinux.jobs["diff-mutants-linux"].strategy["max-parallel"] = 5;
  assert.ok(check(tooManyLinux).includes("strategy:diff-mutants-linux"));

  const failFastEnabled = structuredClone(ci);
  failFastEnabled.jobs["diff-mutants-linux"].strategy["fail-fast"] = true;
  assert.ok(check(failFastEnabled).includes("strategy:diff-mutants-linux"));

  const run = {
    id: 90,
    run_attempt: 1,
    head_sha: HEAD,
    event: "push",
    head_branch: "feature",
    status: "completed",
    conclusion: "success",
    path: CI_PATH,
    repository: { full_name: "gihwan-dev/stock-copilot" },
    head_repository: { full_name: "gihwan-dev/stock-copilot" },
  };
  const baseJobs = [
    makeJob("linux", [
      "container visual regression",
      "linux check",
      "linux tests without network",
    ]),
    makeJob("macos-branch", ["branch checkout", "just check"]),
    makeJob("diff-mutants-plan", ["range", "diff mutants plan"]),
    makeJob("diff-mutants-verdict", ["diff mutants aggregate"]),
  ];
  const badShard = makeJob(
    "diff-mutants-linux (0/2)",
    ["diff mutants shard", "diff mutants summary"],
    { conclusion: "failure" },
  );
  assert.equal(checkRun(HEAD, run, [...baseJobs, badShard]), false);
  assert.equal(
    failureCode(HEAD, { ...run, conclusion: "failure" }, [
      ...baseJobs.slice(0, 3),
      { ...baseJobs[3], conclusion: "failure" },
      badShard,
    ]),
    "CHECK_FAILED",
  );
});
