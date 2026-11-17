import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import {
  recordProblems,
  reviewProblems,
  isLegacyRecord,
} from "../review-records/check.mjs";
import { repoRoot } from "./signature-repo.mjs";

const git = (ref) => {
  const run = spawnSync("git", ["rev-parse", ref], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(run.status, 0);
  return run.stdout.trim();
};

const base = git("HEAD^");
const target = git("HEAD");
const record = (tier, rounds) => `# 리뷰 기록 H-52

## 커밋

| 회차 | 기준 SHA | 대상 SHA |
|---|---|---|
${rounds.map((round) => `| ${round} | ${base} | ${target} |`).join("\n")}

## 리뷰

| 번호 | 등급 | 지적 | 평가 |
|---|---|---|---|
| 1 | 없음 | 없음 | 통과 |

## 측정

| 항목 | 값 |
|---|---|
| 리뷰 회차 | ${rounds.length} |
${tier === undefined ? "" : `| 리뷰 등급 | ${tier} |`}
`;

const problems = (tier, rounds) =>
  recordProblems(repoRoot, record(tier, rounds), "H-52");

test("리뷰 등급 1부터 3까지 각 상한의 실제 회차를 허용한다", () => {
  for (const tier of [1, 2, 3]) {
    assert.deepEqual(
      problems(
        tier,
        Array.from({ length: tier }, (_, i) => `${i + 1}`),
      ),
      [],
    );
  }
});

test("리뷰 등급과 상한을 벗어나거나 등급이 중복된 기록을 거부한다", () => {
  for (const tier of ["0", "4", "1.0", "2회", ""]) {
    assert.notDeepEqual(problems(tier, ["1"]), [], tier);
  }
  for (const tier of [1, 2, 3]) {
    assert.notDeepEqual(
      problems(
        tier,
        Array.from({ length: tier + 1 }, (_, i) => `${i + 1}`),
      ),
      [],
      `${tier}`,
    );
  }
  assert.notDeepEqual(
    recordProblems(
      repoRoot,
      record("1", ["1"]) + "| 리뷰 등급 | 3 |\n",
      "H-52",
    ),
    [],
  );
});

test("마지막 회차 다음의 수정 행만 추가로 허용한다", () => {
  assert.deepEqual(problems(1, ["1", "2 (1차 수정)"]), []);
  assert.deepEqual(problems(3, ["1", "2", "3", "4 (3차 수정)"]), []);
  assert.notDeepEqual(problems(1, ["1", "2 (2차 검토)"]), []);
  assert.notDeepEqual(problems(1, ["1", "2 (1차 수정)", "3"]), []);
  assert.notDeepEqual(problems(3, ["1", "2 (1차 수정)", "3"]), []);
  assert.notDeepEqual(problems(1, ["1", "1"]), []);
  assert.notDeepEqual(problems(3, ["1", "3"]), []);
});

test("기준 커밋에 있던 과거 기록만 원래 내용 그대로 예외를 받는다", async () => {
  const { readFileSync } = await import("node:fs");
  const { join } = await import("node:path");
  const path = "docs/reviews/H-49.md";
  const text = readFileSync(join(repoRoot, path), "utf8");
  assert.equal(isLegacyRecord(repoRoot, path, text), true);
  assert.equal(isLegacyRecord(repoRoot, path, `${text}\n`), false);
  assert.equal(isLegacyRecord(repoRoot, "docs/reviews/X-999.md", text), false);
});

test("등급이 없는 새 기록은 회차 수와 관계없이 거부한다", () => {
  for (const rounds of [["1"], ["1", "2", "3", "4"]]) {
    assert.ok(
      problems(undefined, rounds).some((item) => item.includes("리뷰 등급")),
    );
  }
  const misleading = record(undefined, ["1"]).replace(
    "## 커밋",
    "| 리뷰 등급 | 3 |\n\n## 커밋",
  );
  assert.ok(
    recordProblems(repoRoot, misleading, "H-52").some((item) =>
      item.includes("리뷰 등급"),
    ),
  );
});

test("아직 완료 progress에 연결되지 않은 새 리뷰 기록도 등급을 검사한다", async () => {
  const { mkdirSync, mkdtempSync, rmSync, writeFileSync } =
    await import("node:fs");
  const { join } = await import("node:path");
  const parent = join(repoRoot, "target/h52-tier");
  mkdirSync(parent, { recursive: true });
  const root = mkdtempSync(join(parent, "repo-"));
  try {
    mkdirSync(join(root, "docs/reviews"), { recursive: true });
    writeFileSync(
      join(root, "docs/progress.md"),
      "| 작업 | 상태 | 대상 커밋 | 리뷰 기록 |\n|---|---|---|---|\n| H-52 | 해결 | - | [기록](reviews/H-52.md) |\n",
    );
    writeFileSync(join(root, "docs/reviews/H-52.md"), record("4", ["1"]));
    assert.ok(
      reviewProblems(root).some((problem) => problem.includes("리뷰 등급")),
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("progress에 연결되지 않은 새 기록에도 리뷰 등급이 필요하다", async () => {
  const { mkdirSync, mkdtempSync, rmSync, writeFileSync } =
    await import("node:fs");
  const { join } = await import("node:path");
  const parent = join(repoRoot, "target/h52-tier");
  mkdirSync(parent, { recursive: true });
  const root = mkdtempSync(join(parent, "repo-"));
  try {
    assert.equal(spawnSync("git", ["init", "--quiet", root]).status, 0);
    assert.equal(
      spawnSync("git", [
        "-C",
        root,
        "fetch",
        "--quiet",
        "--no-tags",
        repoRoot,
        target,
      ]).status,
      0,
    );
    mkdirSync(join(root, "docs/reviews"), { recursive: true });
    writeFileSync(
      join(root, "docs/progress.md"),
      "| 작업 | 상태 | 대상 커밋 | 리뷰 기록 |\n|---|---|---|---|\n| H-52 | 해결 | - | - |\n",
    );
    writeFileSync(join(root, "docs/reviews/H-52.md"), record(undefined, ["1"]));
    assert.ok(
      reviewProblems(root).some((problem) => problem.includes("리뷰 등급")),
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
