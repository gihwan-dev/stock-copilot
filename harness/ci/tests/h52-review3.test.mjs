import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { failureCode, remoteCheckFrom } from "../remote-check.mjs";
import { recordProblems } from "../review-records/check.mjs";
import { repoRoot } from "./signature-repo.mjs";

const sha = "a".repeat(40);
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
const job = (name, steps) => ({
  name,
  head_sha: sha,
  run_attempt: 2,
  status: "completed",
  conclusion: "success",
  steps: steps.map((step) => ({
    name: step,
    status: "completed",
    conclusion: "success",
  })),
});
const linux = job("linux", [
  "container visual regression",
  "linux check",
  "linux tests without network",
]);
const macos = job("macos-branch", ["just check"]);
const plan = job("diff-mutants-plan", ["range", "diff mutants plan"]);
const verdict = job("diff-mutants-verdict", ["diff mutants aggregate"]);

test("성공 작업의 필수 단계 누락을 단계 이름과 함께 거부한다", async () => {
  const jobs = [linux, macos, plan, verdict];
  assert.equal(failureCode(sha, run, jobs), "MISSING_REQUIRED_STEPS");
  const read = async (path) =>
    path.includes("workflows/ci.yml/")
      ? { total_count: 1, workflow_runs: [run] }
      : { total_count: jobs.length, jobs };
  await assert.rejects(
    remoteCheckFrom(sha, read),
    (error) =>
      error.code === "MISSING_REQUIRED_STEPS" &&
      /macos-branch.*branch checkout/u.test(error.message),
  );
  assert.equal(
    failureCode(sha, { ...run, head_sha: "b".repeat(40) }, jobs),
    "INVALID_EVIDENCE",
  );
  assert.equal(
    failureCode(sha, { ...run, conclusion: "failure" }, [
      linux,
      { ...macos, conclusion: "failure" },
      plan,
      verdict,
    ]),
    "CHECK_FAILED",
  );
});

const git = (ref) => {
  const result = spawnSync("git", ["rev-parse", ref], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(result.status, 0);
  return result.stdout.trim();
};

test("리뷰 생략에는 실제 회차 0을 적고 리뷰 절을 함께 두지 않는다", () => {
  const before = git("HEAD^");
  const after = git("HEAD");
  const waiver = `# 리뷰 기록 X-999

## 커밋

| 회차 | 기준 SHA | 대상 SHA |
|---|---|---|
| 1 | ${before} | ${after} |

## 리뷰 생략

- 결정: 사용자
- 사유: 문서 내용의 단순 수정이라 독립 리뷰를 생략하기로 했다.

## 측정

| 항목 | 값 |
|---|---|
| 리뷰 등급 | 1 |
| 리뷰 회차 | 0 |
`;
  assert.deepEqual(recordProblems(repoRoot, waiver, "X-999"), []);
  assert.notDeepEqual(
    recordProblems(
      repoRoot,
      waiver.replace("| 리뷰 회차 | 0 |", "| 리뷰 회차 | 1 |"),
      "X-999",
    ),
    [],
  );
  const review =
    "## 리뷰 (회차 99)\n\n| 번호 | 등급 |\n|---|---|\n| 1 | 없음 |\n\n";
  assert.notDeepEqual(
    recordProblems(
      repoRoot,
      waiver.replace("## 리뷰 생략", `${review}## 리뷰 생략`),
      "X-999",
    ),
    [],
  );
  assert.notDeepEqual(
    recordProblems(
      repoRoot,
      waiver.replace("- 결정: 사용자", "- 결정: 구현자"),
      "X-999",
    ),
    [],
  );
});
