import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { recordProblems, tierProblems } from "../review-records/check.mjs";
import { recordedCommits } from "../merge-gate.mjs";
import { failureCode, remoteCheckFrom } from "../remote-check.mjs";
import { repoRoot } from "./signature-repo.mjs";

const record = (rows, headings, measured) => `# 리뷰 기록 X-999

## 커밋

| 회차 | 기준 SHA | 대상 SHA |
|---|---|---|
${rows}

${headings}

## 측정

| 항목 | 값 |
|---|---|
| 리뷰 등급 | 1 |
| 리뷰 회차 | ${measured} |
`;

test("커밋 표와 실제 리뷰 절 및 측정 회차의 모순을 거부한다", () => {
  const head = "a".repeat(40);
  const row = (round) => `| ${round} | ${head} | ${head} |`;
  const headings = [1, 2, 3]
    .map(
      (round) =>
        `## 리뷰 (회차 ${round})\n\n| 번호 | 등급 |\n|---|---|\n| 1 | 없음 |`,
    )
    .join("\n\n");
  assert.notDeepEqual(
    tierProblems(
      record(
        [row("1"), row("2 (재리뷰 수정)"), row("3 (재리뷰 수정)")].join("\n"),
        headings,
        1,
      ),
    ),
    [],
  );
  assert.notDeepEqual(tierProblems(record(row("1"), headings, 3)), []);
  assert.notDeepEqual(
    tierProblems(record(row("1"), "## 리뷰 (회차 1)", 2)),
    [],
  );
  assert.notDeepEqual(
    tierProblems(record(row("1"), "## 리뷰 (회차 2)", 1)),
    [],
  );
  assert.deepEqual(tierProblems(record(row("1"), "## 리뷰 (회차 1)", 1)), []);
});

const fixture = () => {
  const parent = join(repoRoot, "target/h52-review2");
  mkdirSync(parent, { recursive: true });
  const root = mkdtempSync(join(parent, "repo-"));
  const env = {
    ...process.env,
    GIT_CONFIG_GLOBAL: "/dev/null",
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_AUTHOR_NAME: "Test",
    GIT_AUTHOR_EMAIL: "test@example.invalid",
    GIT_COMMITTER_NAME: "Test",
    GIT_COMMITTER_EMAIL: "test@example.invalid",
  };
  const git = (...args) => {
    const result = spawnSync("git", args, { cwd: root, env, encoding: "utf8" });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  const put = (path, value) => {
    mkdirSync(join(root, path, ".."), { recursive: true });
    writeFileSync(join(root, path), value);
  };
  git("init", "--quiet", "-b", "master");
  put("app.rs", "base\n");
  put(
    "harness/agent-writable.txt",
    "app.rs\ndocs/**\ncrates/*/src/**\ncrates/*/Cargo.toml\n!crates/xtask/**\n",
  );
  git("add", "app.rs", "harness/agent-writable.txt");
  git("commit", "--quiet", "-m", "base");
  const base = git("rev-parse", "HEAD");
  put("app.rs", "feature\n");
  git("add", "app.rs");
  git("commit", "--quiet", "-m", "feature");
  const head = git("rev-parse", "HEAD");
  return {
    root,
    git,
    put,
    base,
    head,
    close: () => rmSync(root, { recursive: true, force: true }),
  };
};

test("중첩 괄호가 있는 리뷰 행에서도 두 커밋을 원격 도달 검사에 보낸다", () => {
  const made = fixture();
  try {
    made.put(
      "docs/reviews/X-999.md",
      `# X-999\n\n## 커밋\n\n| 회차 | 기준 SHA | 대상 SHA |\n|---|---|---|\n| 1 (수정 (재)) | ${made.base} | ${made.head} |\n`,
    );
    made.put(
      "docs/progress.md",
      "| 작업 | 상태 | 대상 커밋 | 리뷰 기록 |\n|---|---|---|---|\n",
    );
    assert.deepEqual(recordedCommits(made.root), [made.base, made.head]);
  } finally {
    made.close();
  }
});

test("같은 커밋의 여러 리뷰도 회차마다 표를 기록한다", () => {
  const made = fixture();
  try {
    const rows = [1, 2]
      .map((round) => `| ${round} | ${made.base} | ${made.head} |`)
      .join("\n");
    const heading = (round) =>
      `## 리뷰 (회차 ${round})\n\n| 번호 | 등급 |\n|---|---|\n| 1 | 없음 |`;
    const text = record(rows, `${heading(1)}\n\n${heading(2)}`, 2).replace(
      "| 리뷰 등급 | 1 |",
      "| 리뷰 등급 | 2 |",
    );
    assert.deepEqual(recordProblems(made.root, text, "X-999"), []);
    assert.notDeepEqual(
      recordProblems(
        made.root,
        text.replace(heading(2), "## 리뷰 (회차 2)"),
        "X-999",
      ),
      [],
    );
  } finally {
    made.close();
  }
});

test("검사 정의가 기준 master와 다른 브랜치의 원격 증거를 거부한다", async () => {
  const { assertTrustedExecution, remoteCheck, remoteLinuxCheck } =
    await import("../remote-check.mjs");
  const made = fixture();
  try {
    assert.doesNotThrow(() =>
      assertTrustedExecution(made.root, made.base, made.head),
    );
    for (const path of [
      ".github/workflows/ci.yml",
      ".github/workflows/signatures.yml",
      "harness/ci/workflow-policy/policy.json",
      "justfile",
      "crates/xtask/src/main.rs",
      "crates/domain/Cargo.toml",
    ]) {
      made.git("reset", "--quiet", "--hard", made.head);
      made.put(path, "changed\n");
      made.git("add", "--", path);
      made.git("commit", "--quiet", "-m", path);
      const changed = made.git("rev-parse", "HEAD");
      assert.throws(
        () => assertTrustedExecution(made.root, made.base, changed),
        (error) => error.message.includes(path),
        path,
      );
      assert.throws(
        () => remoteCheck(changed, made.root, made.base),
        (error) => error.message.includes(path),
        path,
      );
      assert.throws(
        () => remoteLinuxCheck(changed, made.root, made.base),
        (error) => error.message.includes(path),
        path,
      );
    }
    assert.throws(
      () => assertTrustedExecution(made.root, made.base, "f".repeat(40)),
      /검사 정의/u,
    );
  } finally {
    made.close();
  }
});

test("같은 SHA의 실행 후보 하나라도 ID가 없으면 오래된 성공을 쓰지 않는다", async () => {
  const sha = "a".repeat(40);
  const run = {
    id: 1,
    run_attempt: 1,
    head_sha: sha,
    event: "push",
    head_branch: "feature",
    status: "completed",
    conclusion: "success",
    path: ".github/workflows/ci.yml",
    repository: { full_name: "gihwan-dev/stock-copilot" },
    head_repository: { full_name: "gihwan-dev/stock-copilot" },
  };
  const read = async (path) => {
    if (path.includes("workflows/ci.yml/"))
      return {
        total_count: 2,
        workflow_runs: [run, { ...run, id: undefined, conclusion: "failure" }],
      };
    if (path.includes("workflows/signatures.yml/"))
      return {
        total_count: 1,
        workflow_runs: [
          { ...run, id: 2, path: ".github/workflows/signatures.yml" },
        ],
      };
    const names = path.includes("runs/1/")
      ? ["linux", "macos-branch"]
      : ["signatures"];
    const steps = {
      linux: [
        "container visual regression",
        "linux check",
        "linux tests without network",
      ],
      "macos-branch": ["just check", "diff mutants"],
      signatures: ["signatures from the base checkout"],
    };
    return {
      total_count: names.length,
      jobs: names.map((name) => ({
        name,
        status: "completed",
        conclusion: "success",
        head_sha: sha,
        run_attempt: 1,
        steps: steps[name].map((step) => ({
          name: step,
          status: "completed",
          conclusion: "success",
        })),
      })),
    };
  };
  await assert.rejects(remoteCheckFrom(sha, read), /ID|회차/u);
});

test("미배정 정황에 단계가 null이면 원인을 확정하지 않는다", () => {
  const sha = "a".repeat(40);
  const run = {
    id: 1,
    run_attempt: 1,
    head_sha: sha,
    event: "push",
    head_branch: "feature",
    status: "completed",
    conclusion: "failure",
    path: ".github/workflows/ci.yml",
    repository: { full_name: "gihwan-dev/stock-copilot" },
    head_repository: { full_name: "gihwan-dev/stock-copilot" },
  };
  const linux = {
    name: "linux",
    status: "completed",
    conclusion: "success",
    head_sha: sha,
    run_attempt: 1,
    steps: [],
  };
  const unknown = {
    name: "macos-branch",
    status: "completed",
    conclusion: "failure",
    head_sha: sha,
    run_attempt: 1,
    runner_id: 0,
    runner_name: "",
    steps: null,
  };
  assert.equal(failureCode(sha, run, [linux, unknown]), "INVALID_EVIDENCE");
  assert.equal(
    failureCode(sha, run, [
      { ...linux, conclusion: "failure", steps: [{ conclusion: "failure" }] },
      unknown,
    ]),
    "CHECK_FAILED",
  );
});
