import { mkdtempSync } from "../test-temp.mjs";
import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  checkRun,
  evidenceFor,
  remoteCheckFrom,
  checkLinuxRun,
  remoteLinuxCheckFrom,
  failureCode,
} from "../remote-check.mjs";

const sha = "a".repeat(40);
const other = "b".repeat(40);
const required = [
  "linux",
  "macos-branch",
  "diff-mutants-plan",
  "diff-mutants-verdict",
];
const run = {
  id: 31,
  run_attempt: 2,
  head_sha: sha,
  event: "push",
  head_branch: "feature",
  status: "completed",
  conclusion: "success",
  path: ".github/workflows/ci.yml",
  repository: { full_name: "gihwan-dev/stock-copilot" },
  head_repository: { full_name: "gihwan-dev/stock-copilot" },
};
const stepMap = {
  linux: [
    "container visual regression",
    "linux check",
    "linux tests without network",
  ],
  "macos-branch": ["branch checkout", "just check"],
  macos: ["just check"],
  "diff-mutants-plan": ["range", "diff mutants plan"],
  "diff-mutants-verdict": ["diff mutants aggregate"],
};
const makeJob = (name, steps = stepMap[name]) => ({
  name,
  status: "completed",
  conclusion: "success",
  run_attempt: 2,
  head_sha: sha,
  steps: steps.map((step) => ({
    name: step,
    status: "completed",
    conclusion: "success",
  })),
});
const jobs = required.map((name) => makeJob(name));
const check = (candidate = run, entries = jobs, target = sha) =>
  checkRun(target, candidate, entries);
const replaceJob = (index, override) =>
  jobs.map((entry, position) => (position === index ? override : entry));

test("같은 SHA의 push 실행에서 모든 필수 작업이 성공한 경우만 승인한다", () => {
  assert.equal(check(), true);
  assert.equal(check(run, jobs, other), false);
  assert.equal(check({ ...run, head_sha: other }), false);
  for (const conclusion of [
    "failure",
    "cancelled",
    "skipped",
    "timed_out",
    null,
  ]) {
    assert.equal(check({ ...run, conclusion }), false, `${conclusion}`);
    assert.equal(
      check(run, replaceJob(0, { ...jobs[0], conclusion })),
      false,
      `${conclusion}`,
    );
  }
  assert.equal(
    check({ ...run, status: "in_progress", conclusion: null }),
    false,
  );
  assert.equal(check({ ...run, status: "in_progress" }), false);
  assert.equal(check(run, [jobs[0]]), false);
  assert.equal(check(run, null), false);
  assert.equal(check(run, [...jobs, jobs[0]]), false);
  assert.equal(
    check(run, replaceJob(0, { ...jobs[0], status: "in_progress" })),
    false,
  );
  assert.equal(
    check(run, replaceJob(1, { ...jobs[1], head_sha: other })),
    false,
  );
  assert.equal(
    check(run, replaceJob(1, { ...jobs[1], run_attempt: 1 })),
    false,
  );
});

test("필수 단계가 빠지거나 실패한 과거 실행은 성공 작업이어도 거부한다", () => {
  assert.equal(
    check(run, replaceJob(1, { ...jobs[1], steps: jobs[1].steps.slice(0, 1) })),
    false,
  );
  assert.equal(check(run, replaceJob(0, { ...jobs[0], steps: [] })), false);
  assert.equal(
    check(run, replaceJob(1, { ...jobs[1], steps: undefined })),
    false,
  );
  assert.equal(
    check(
      run,
      replaceJob(2, {
        ...jobs[2],
        steps: jobs[2].steps.map((step) =>
          step.name === "diff mutants plan"
            ? { ...step, status: "in_progress" }
            : step,
        ),
      }),
    ),
    false,
  );
  assert.equal(
    check(
      run,
      replaceJob(3, {
        ...jobs[3],
        steps: jobs[3].steps.map((step) =>
          step.name === "diff mutants aggregate"
            ? { ...step, conclusion: "skipped" }
            : step,
        ),
      }),
    ),
    false,
  );
  assert.equal(
    check(
      run,
      replaceJob(1, {
        ...jobs[1],
        steps: [...jobs[1].steps, jobs[1].steps[0]],
      }),
    ),
    false,
  );
  const signatureRun = { ...run, path: ".github/workflows/signatures.yml" };
  const signatureJob = {
    ...jobs[0],
    name: "signatures",
    steps: [
      {
        name: "signatures inactive",
        status: "completed",
        conclusion: "success",
      },
    ],
  };
  assert.equal(
    checkRun(sha, signatureRun, [signatureJob], {
      file: "signatures.yml",
      jobs: ["signatures"],
    }),
    false,
  );
  signatureJob.steps = [
    {
      name: "signatures from the base checkout",
      status: "completed",
      conclusion: "success",
    },
  ];
  assert.equal(
    checkRun(sha, signatureRun, [signatureJob], {
      file: "signatures.yml",
      jobs: ["signatures"],
    }),
    true,
  );
});

test("원격 조회는 같은 SHA의 최신 실행과 모든 필수 워크플로를 확인한다", async () => {
  const signatureRun = {
    ...run,
    id: 43,
    path: ".github/workflows/signatures.yml",
  };
  const signatureJob = {
    ...jobs[0],
    name: "signatures",
    steps: [
      {
        name: "signatures from the base checkout",
        status: "completed",
        conclusion: "success",
      },
    ],
  };
  const answers = {
    ci: { total_count: 1, workflow_runs: [run] },
    signatures: { total_count: 1, workflow_runs: [signatureRun] },
    ciJobs: { total_count: jobs.length, jobs },
    signatureJobs: { total_count: 1, jobs: [signatureJob] },
  };
  const read = async (path) => {
    if (path.includes("workflows/ci.yml/")) return answers.ci;
    if (path.includes("workflows/signatures.yml/")) return answers.signatures;
    if (path.includes("runs/31/")) return answers.ciJobs;
    if (path.includes("runs/32/")) return answers.ciJobs;
    if (path.includes("runs/43/")) return answers.signatureJobs;
    throw new Error("unexpected request");
  };
  assert.equal(await remoteCheckFrom(sha, read), true);
  answers.ci = {
    total_count: 2,
    workflow_runs: [run, { ...run, id: 32, conclusion: "failure" }],
  };
  await assert.rejects(remoteCheckFrom(sha, read));
  answers.ci = { total_count: 2, workflow_runs: [run] };
  await assert.rejects(remoteCheckFrom(sha, read));
  answers.ci = { total_count: 1, workflow_runs: [run] };
  answers.signatureJobs = { total_count: 0, jobs: [] };
  await assert.rejects(remoteCheckFrom(sha, read));
  await assert.rejects(remoteCheckFrom(other, read));
});

test("실행 출처와 이벤트가 다르면 같은 SHA의 성공도 거부한다", () => {
  assert.equal(check({ ...run, event: "pull_request" }), false);
  assert.equal(check({ ...run, path: ".github/workflows/other.yml" }), false);
  assert.equal(
    check({ ...run, repository: { full_name: "other/project" } }),
    false,
  );
  assert.equal(
    check({ ...run, head_repository: { full_name: "fork/project" } }),
    false,
  );
  assert.equal(check({ ...run, head_branch: "" }), false);
  assert.equal(check({ ...run, head_branch: "master" }), false);
  assert.equal(
    check({ ...run, head_branch: "master" }, replaceJob(1, makeJob("macos"))),
    true,
  );
});

test("영수증의 주장만으로는 성공을 만들 수 없다", () => {
  const receipt = { head: sha, recipe: "check", exit: 0 };
  assert.equal(
    evidenceFor(sha, { receipt, remote: false, signed: false }),
    false,
  );
  assert.equal(
    evidenceFor(other, { receipt, remote: true, signed: false }),
    false,
  );
  assert.equal(
    evidenceFor(sha, { receipt, remote: false, signed: true }),
    true,
  );
  assert.equal(
    evidenceFor(sha, {
      receipt: { ...receipt, exit: 1 },
      remote: false,
      signed: true,
    }),
    false,
  );
  assert.equal(
    evidenceFor(sha, {
      receipt: { ...receipt, recipe: "check-fast" },
      remote: false,
      signed: true,
    }),
    false,
  );
  assert.equal(
    evidenceFor("invalid", { receipt: undefined, remote: true, signed: false }),
    false,
  );
  assert.equal(
    evidenceFor(sha, { receipt: undefined, remote: true, signed: false }),
    true,
  );
});

test("로컬 영수증은 신뢰된 서명과 동일한 내용에만 유효하다", async () => {
  const { spawnSync } = await import("node:child_process");
  const { mkdirSync, readFileSync, rmSync, writeFileSync } =
    await import("node:fs");
  const { join } = await import("node:path");
  const { repoRoot } = await import("./signature-repo.mjs");
  const { verifyReceipt } = await import("../remote-check.mjs");
  const parent = join(repoRoot, "target/h52-signatures");
  mkdirSync(parent, { recursive: true });
  const directory = mkdtempSync(join(parent, "case-"));
  try {
    const key = join(directory, "key");
    const file = join(directory, "final-check.json");
    const generated = spawnSync(
      "ssh-keygen",
      ["-q", "-t", "ed25519", "-N", "", "-f", key],
      { encoding: "utf8" },
    );
    assert.equal(generated.status, 0, generated.stderr);
    const signers = `owner ${readFileSync(`${key}.pub`, "utf8")}`;
    writeFileSync(
      file,
      JSON.stringify({ head: sha, recipe: "check", exit: 0 }),
    );
    const signed = spawnSync(
      "ssh-keygen",
      ["-Y", "sign", "-f", key, "-n", "stock-copilot-check", file],
      { encoding: "utf8" },
    );
    assert.equal(signed.status, 0, signed.stderr);
    assert.equal(verifyReceipt(file, signers, "owner", directory), true);
    assert.equal(
      verifyReceipt(file, signers.trimEnd(), "owner", directory),
      true,
    );
    assert.equal(verifyReceipt(file, "", "owner", directory), false);
    assert.equal(verifyReceipt(file, signers, "other", directory), false);
    writeFileSync(
      file,
      JSON.stringify({ head: other, recipe: "check", exit: 0 }),
    );
    assert.equal(verifyReceipt(file, signers, "owner", directory), false);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("러너 미배정·취소·시간 초과는 통과가 아닌 재시도 대상으로 구분한다", async () => {
  const failedRun = { ...run, conclusion: "failure" };
  const unassigned = {
    ...jobs[1],
    runner_id: 0,
    runner_name: "",
    steps: [],
    conclusion: "cancelled",
  };
  const withUnassigned = replaceJob(1, unassigned);
  assert.equal(checkRun(sha, failedRun, withUnassigned), false);
  assert.equal(
    failureCode(sha, failedRun, withUnassigned),
    "RETRY_RUNNER_UNASSIGNED",
  );
  assert.equal(
    failureCode(
      sha,
      failedRun,
      replaceJob(1, {
        ...jobs[1],
        conclusion: "cancelled",
        runner_id: 11,
        runner_name: "hosted",
      }),
    ),
    "RETRY_CANCELLED",
  );
  assert.equal(
    failureCode(
      sha,
      failedRun,
      replaceJob(1, {
        ...jobs[1],
        conclusion: "timed_out",
        runner_id: 12,
        runner_name: "hosted",
      }),
    ),
    "RETRY_TIMED_OUT",
  );
  assert.equal(
    failureCode(
      sha,
      failedRun,
      replaceJob(1, {
        ...jobs[1],
        conclusion: "failure",
        runner_id: 12,
        runner_name: "hosted",
        steps: [
          { name: "just check", status: "completed", conclusion: "timed_out" },
        ],
      }),
    ),
    "RETRY_TIMED_OUT",
  );
  assert.equal(
    failureCode(
      sha,
      { ...run, status: "queued", conclusion: null },
      replaceJob(1, { ...unassigned, status: "queued", conclusion: null }),
    ),
    "PENDING",
  );
  const failingLinux = {
    ...jobs[0],
    conclusion: "failure",
    runner_id: 13,
    runner_name: "hosted",
    steps: [
      { name: "linux check", status: "completed", conclusion: "failure" },
    ],
  };
  assert.equal(
    failureCode(sha, failedRun, [failingLinux, unassigned, jobs[2], jobs[3]]),
    "CHECK_FAILED",
  );
  assert.equal(
    failureCode(other, failedRun, withUnassigned),
    "INVALID_EVIDENCE",
  );
  const read = async (path) =>
    path.includes("workflows/ci.yml/")
      ? { total_count: 1, workflow_runs: [failedRun] }
      : { total_count: withUnassigned.length, jobs: withUnassigned };
  await assert.rejects(
    remoteCheckFrom(sha, read),
    (error) => error.code === "RETRY_RUNNER_UNASSIGNED",
  );
});

test("변이 시간 제한은 코드 실패와 분리하되 성공으로 인정하지 않는다", () => {
  const failedRun = { ...run, conclusion: "failure" };
  const failedVerdict = {
    ...jobs[3],
    conclusion: "failure",
    steps: [
      {
        name: "diff mutants aggregate",
        status: "completed",
        conclusion: "failure",
      },
    ],
  };
  const timedOutShard = {
    name: "diff-mutants-linux (0/2)",
    status: "completed",
    conclusion: "failure",
    run_attempt: 2,
    head_sha: sha,
    runner_id: 21,
    runner_name: "hosted",
    steps: [
      {
        name: "diff mutants shard",
        status: "completed",
        conclusion: "timed_out",
      },
      {
        name: "diff mutants summary",
        status: "completed",
        conclusion: "failure",
      },
    ],
  };
  const passedShard = makeJob("diff-mutants-linux (1/2)", [
    "diff mutants shard",
    "diff mutants summary",
  ]);
  const withTimeout = [
    jobs[0],
    jobs[1],
    jobs[2],
    failedVerdict,
    timedOutShard,
    passedShard,
  ];
  assert.equal(checkRun(sha, failedRun, withTimeout), false);
  assert.equal(
    failureCode(sha, failedRun, withTimeout),
    "RETRY_MUTATION_TIMED_OUT",
  );
  const codeFailedShard = {
    ...timedOutShard,
    steps: [
      {
        name: "diff mutants shard",
        status: "completed",
        conclusion: "failure",
      },
      {
        name: "diff mutants summary",
        status: "completed",
        conclusion: "failure",
      },
    ],
  };
  assert.equal(
    failureCode(sha, failedRun, [
      jobs[0],
      jobs[1],
      jobs[2],
      failedVerdict,
      codeFailedShard,
      passedShard,
    ]),
    "CHECK_FAILED",
  );
  assert.equal(
    failureCode(sha, failedRun, [
      jobs[0],
      jobs[1],
      jobs[2],
      failedVerdict,
      passedShard,
    ]),
    "CHECK_FAILED",
  );
  assert.equal(
    failureCode(sha, failedRun, [
      jobs[0],
      jobs[1],
      jobs[2],
      failedVerdict,
      {
        ...timedOutShard,
        runner_id: 0,
        runner_name: "",
        steps: [],
        conclusion: "cancelled",
      },
      passedShard,
    ]),
    "RETRY_RUNNER_UNASSIGNED",
  );
});

test("macOS가 대기·실패해도 Linux 작업만의 성공은 별도로 확인한다", async () => {
  const failedRun = { ...run, conclusion: "failure" };
  const macos = {
    ...jobs[1],
    runner_id: 0,
    runner_name: "",
    steps: [],
    conclusion: "cancelled",
  };
  const entries = [jobs[0], macos];
  assert.equal(checkRun(sha, failedRun, entries), false);
  assert.equal(checkLinuxRun(sha, failedRun, entries), true);
  assert.equal(
    checkLinuxRun(
      sha,
      { ...failedRun, status: "queued", conclusion: null },
      entries,
    ),
    true,
  );
  assert.equal(checkLinuxRun(other, failedRun, entries), false);
  assert.equal(
    checkLinuxRun(sha, { ...failedRun, status: "unknown" }, entries),
    false,
  );
  assert.equal(
    checkLinuxRun(sha, { ...failedRun, event: "pull_request" }, entries),
    false,
  );
  assert.equal(
    checkLinuxRun(
      sha,
      { ...failedRun, repository: { full_name: "fork/project" } },
      entries,
    ),
    false,
  );
  assert.equal(checkLinuxRun(sha, failedRun, [jobs[0], jobs[0], macos]), false);
  assert.equal(
    checkLinuxRun(sha, failedRun, [
      { ...jobs[0], status: "in_progress" },
      macos,
    ]),
    false,
  );
  assert.equal(checkLinuxRun(sha, failedRun, null), false);
  assert.equal(
    checkLinuxRun(sha, failedRun, [
      { ...jobs[0], conclusion: "failure" },
      macos,
    ]),
    false,
  );
  assert.equal(
    checkLinuxRun(sha, failedRun, [
      { ...jobs[0], steps: jobs[0].steps.slice(1) },
      macos,
    ]),
    false,
  );
  assert.equal(
    checkLinuxRun(sha, failedRun, [{ ...jobs[0], run_attempt: 1 }, macos]),
    false,
  );
  const read = async (path) =>
    path.includes("workflows/ci.yml/")
      ? { total_count: 1, workflow_runs: [failedRun] }
      : { total_count: 2, jobs: entries };
  const linuxEvidence = await remoteLinuxCheckFrom(sha, read);
  assert.deepEqual(linuxEvidence, {
    scope: "linux",
    head: sha,
    runId: 31,
    attempt: 2,
  });
  assert.equal(
    evidenceFor(sha, {
      remote: linuxEvidence,
      receipt: undefined,
      signed: false,
    }),
    false,
  );
  await assert.rejects(remoteCheckFrom(sha, read));
});
