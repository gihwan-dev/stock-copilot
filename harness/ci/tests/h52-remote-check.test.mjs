import { strict as assert } from "node:assert";
import { test } from "node:test";
import { checkRun, evidenceFor, remoteCheckFrom } from "../remote-check.mjs";

const sha = "a".repeat(40);
const other = "b".repeat(40);
const required = ["linux", "macos-branch"];
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
const jobs = required.map((name) => ({
  name,
  status: "completed",
  conclusion: "success",
  run_attempt: 2,
  head_sha: sha,
  steps: (name === "linux"
    ? [
        "container visual regression",
        "linux check",
        "linux tests without network",
      ]
    : ["just check", "diff mutants"]
  ).map((step) => ({ name: step, status: "completed", conclusion: "success" })),
}));
const check = (candidate = run, entries = jobs, target = sha) =>
  checkRun(target, candidate, entries);

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
      check(run, [{ ...jobs[0], conclusion }, jobs[1]]),
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
    check(run, [{ ...jobs[0], status: "in_progress" }, jobs[1]]),
    false,
  );
  assert.equal(check(run, [jobs[0], { ...jobs[1], head_sha: other }]), false);
  assert.equal(check(run, [jobs[0], { ...jobs[1], run_attempt: 1 }]), false);
});

test("필수 단계가 빠지거나 실패한 과거 실행은 성공 작업이어도 거부한다", () => {
  assert.equal(
    check(run, [jobs[0], { ...jobs[1], steps: jobs[1].steps.slice(0, 1) }]),
    false,
  );
  assert.equal(check(run, [{ ...jobs[0], steps: [] }, jobs[1]]), false);
  assert.equal(check(run, [jobs[0], { ...jobs[1], steps: undefined }]), false);
  assert.equal(
    check(run, [
      jobs[0],
      {
        ...jobs[1],
        steps: jobs[1].steps.map((step) =>
          step.name === "diff mutants"
            ? { ...step, status: "in_progress" }
            : step,
        ),
      },
    ]),
    false,
  );
  assert.equal(
    check(run, [
      jobs[0],
      {
        ...jobs[1],
        steps: jobs[1].steps.map((step) =>
          step.name === "diff mutants"
            ? { ...step, conclusion: "skipped" }
            : step,
        ),
      },
    ]),
    false,
  );
  assert.equal(
    check(run, [
      jobs[0],
      { ...jobs[1], steps: [...jobs[1].steps, jobs[1].steps[0]] },
    ]),
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
    ciJobs: { total_count: 2, jobs },
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
    check({ ...run, head_branch: "master" }, [
      jobs[0],
      { ...jobs[1], name: "macos" },
    ]),
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
  const { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } =
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
