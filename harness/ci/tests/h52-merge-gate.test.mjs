import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  existsSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { inspectMerge } from "../merge-gate.mjs";
import { repoRoot } from "./signature-repo.mjs";

const scratch = join(repoRoot, "target", "h52-merge-tests");
mkdirSync(scratch, { recursive: true });

const fixture = ({ withGate = false } = {}) => {
  const directory = mkdtempSync(join(scratch, "repo-"));
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
    const p = spawnSync("git", args, { cwd: directory, env, encoding: "utf8" });
    assert.equal(p.status, 0, p.stderr);
    return p.stdout.trim();
  };
  const put = (path, text) => {
    const file = join(directory, path);
    mkdirSync(join(file, ".."), { recursive: true });
    writeFileSync(file, text);
    git("add", "--", path);
  };
  git("init", "--quiet", "-b", "master");
  put("app.rs", "base\n");
  if (withGate) {
    for (const path of [
      "harness/ci/merge-gate.mjs",
      "harness/ci/remote-check.mjs",
      "harness/ci/review-records/check.mjs",
      ".claude/hooks/lib/allowlist.mjs",
    ]) {
      put(path, readFileSync(join(repoRoot, path), "utf8"));
    }
  }
  git("commit", "--quiet", "-m", "base");
  git("checkout", "--quiet", "-b", "feature");
  put("app.rs", "feature\n");
  git("commit", "--quiet", "-m", "feature");
  const head = git("rev-parse", "HEAD");
  git("checkout", "--quiet", "master");
  git("merge", "--quiet", "--no-ff", "--no-commit", "feature");
  return {
    directory,
    git,
    put,
    head,
    close: () => rmSync(directory, { recursive: true, force: true }),
  };
};

const inspect = (made) => inspectMerge(made.directory, "feature");

test("현재 master를 포함한 브랜치와 문서·생성 등록만 다른 병합을 허용한다", () => {
  const made = fixture();
  try {
    made.put("docs/reviews/H-52.md", "기록\n");
    made.put("harness/sandbox/lock-fragment.json", "{}\n");
    const result = inspect(made);
    assert.equal(result.problem, undefined);
    assert.equal(result.head, made.head);
  } finally {
    made.close();
  }
});

test("옆가지 리뷰 커밋은 일시 브랜치가 아닌 원격 태그가 필요하다", async () => {
  const { advertisedTags, missingRemoteObjects } =
    await import("../merge-gate.mjs");
  const made = fixture();
  try {
    made.git("merge", "--abort");
    made.git("checkout", "--quiet", "-b", "side");
    made.put("side.txt", "side\n");
    made.git("commit", "--quiet", "-m", "side");
    const side = made.git("rev-parse", "HEAD");
    const branchOnly = advertisedTags(`${side}\trefs/heads/temporary\n`);
    assert.deepEqual(
      missingRemoteObjects(made.directory, [side], made.head, branchOnly),
      [side],
    );
    const tagged = advertisedTags(`${side}\trefs/tags/review-evidence/side\n`);
    assert.deepEqual(
      missingRemoteObjects(made.directory, [side], made.head, tagged),
      [],
    );
  } finally {
    made.close();
  }
});

test("병합 중단 뒤 남은 요청은 다음 커밋 가드에서 지운다", () => {
  const made = fixture();
  try {
    const state = inspect(made);
    const marker = join(made.directory, ".git/stock-copilot-merge-gate.json");
    writeFileSync(marker, JSON.stringify({ branch: "feature", ...state }));
    made.git("merge", "--abort");
    const program = join(repoRoot, "harness/ci/merge-gate.mjs");
    const result = spawnSync(
      "node",
      [program, "--prepare-hook", ".git/index"],
      { cwd: made.directory, encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr);
    assert.equal(existsSync(marker), false);
  } finally {
    made.close();
  }
});

test("없는 리뷰 커밋은 기록 파일과 행 및 SHA를 알려 준다", async () => {
  const { recordedCommits } = await import("../merge-gate.mjs");
  const made = fixture();
  try {
    made.put(
      "docs/reviews/H-52.md",
      "# H-52\n\n## 커밋\n\n| 회차 | 기준 SHA | 대상 SHA |\n|---|---|---|\n| 1 | deadbeef | deadbeef |\n",
    );
    made.put(
      "docs/progress.md",
      "| 작업 | 상태 | 대상 커밋 | 리뷰 기록 |\n|---|---|---|---|\n",
    );
    assert.throws(
      () => recordedCommits(made.directory),
      /docs\/reviews\/H-52\.md:7.*deadbeef/u,
    );
  } finally {
    made.close();
  }
});

test("병합 인덱스에 소스가 다르거나 작업 사본이 인덱스와 다르면 거부한다", () => {
  const made = fixture();
  try {
    made.put("app.rs", "changed after test\n");
    assert.match(inspect(made).problem, /허용되지 않은/u);
    made.put("app.rs", "feature\n");
    writeFileSync(join(made.directory, "app.rs"), "unstaged\n");
    assert.match(inspect(made).problem, /작업 사본/u);
  } finally {
    made.close();
  }
});

test("master를 포함하지 않은 작업 브랜치는 거부한다", () => {
  const made = fixture();
  try {
    made.git("merge", "--abort");
    made.put("master.txt", "advance\n");
    made.git("commit", "--quiet", "-m", "advance master");
    const result = inspect(made);
    assert.match(result.problem, /master.*포함/u);
  } finally {
    made.close();
  }
});

test("브랜치에 없는 리뷰 커밋은 실제 원격 참조에서 도달해야 한다", async () => {
  const { missingRemoteObjects } = await import("../merge-gate.mjs");
  const made = fixture();
  try {
    made.git("merge", "--abort");
    made.git("checkout", "--quiet", "-b", "side");
    made.put("side.txt", "side\n");
    made.git("commit", "--quiet", "-m", "side");
    const side = made.git("rev-parse", "HEAD");
    assert.deepEqual(
      missingRemoteObjects(made.directory, [side], made.head, []),
      [side],
    );
    assert.deepEqual(
      missingRemoteObjects(made.directory, [side], made.head, [side]),
      [],
    );
    assert.deepEqual(
      missingRemoteObjects(made.directory, [side], made.head, ["c".repeat(40)]),
      [side],
    );
  } finally {
    made.close();
  }
});

test("커밋 훅이 전달한 실제 인덱스에 다른 소스가 있으면 거부한다", async () => {
  const { copyFileSync } = await import("node:fs");
  const made = fixture();
  try {
    const index = join(made.directory, ".git/index");
    const alternate = join(made.directory, ".git/other-index");
    copyFileSync(index, alternate);
    writeFileSync(
      join(made.directory, "app.rs"),
      "unreviewed commit -a content\n",
    );
    const add = spawnSync("git", ["add", "app.rs"], {
      cwd: made.directory,
      encoding: "utf8",
      env: {
        ...process.env,
        GIT_CONFIG_GLOBAL: "/dev/null",
        GIT_CONFIG_NOSYSTEM: "1",
        GIT_INDEX_FILE: alternate,
      },
    });
    assert.equal(add.status, 0, add.stderr);
    const result = inspectMerge(made.directory, "feature", alternate);
    assert.match(result.problem, /허용되지 않은/u);
  } finally {
    made.close();
  }
});

test("커밋 가드가 실제 인덱스를 게이트 요청과 비교해 등록한다", () => {
  const made = fixture();
  try {
    const state = inspect(made);
    const marker = join(made.directory, ".git/stock-copilot-merge-gate.json");
    writeFileSync(marker, JSON.stringify({ branch: "feature", ...state }));
    const program = join(repoRoot, "harness/ci/merge-gate.mjs");
    const prepared = spawnSync(
      "node",
      [program, "--prepare-hook", ".git/index"],
      {
        cwd: made.directory,
        encoding: "utf8",
        env: {
          ...process.env,
          GIT_CONFIG_GLOBAL: "/dev/null",
          GIT_CONFIG_NOSYSTEM: "1",
        },
      },
    );
    assert.equal(prepared.status, 0, prepared.stderr);
    const stored = JSON.parse(readFileSync(marker, "utf8"));
    assert.equal(stored.index, join(made.directory, ".git/index"));
    writeFileSync(marker, JSON.stringify({ ...stored, tree: "a".repeat(40) }));
    const changed = spawnSync(
      "node",
      [program, "--prepare-hook", ".git/index"],
      {
        cwd: made.directory,
        encoding: "utf8",
        env: {
          ...process.env,
          GIT_CONFIG_GLOBAL: "/dev/null",
          GIT_CONFIG_NOSYSTEM: "1",
        },
      },
    );
    assert.equal(changed.status, 1);
    const blocked = spawnSync("node", [program, "--prepare-hook", ""], {
      cwd: made.directory,
      encoding: "utf8",
      env: {
        ...process.env,
        GIT_CONFIG_GLOBAL: "/dev/null",
        GIT_CONFIG_NOSYSTEM: "1",
      },
    });
    assert.equal(blocked.status, 1);
  } finally {
    made.close();
  }
});

test("실제 xtask 커밋 가드가 준비된 병합의 커밋 인덱스를 검증한다", () => {
  const made = fixture({ withGate: true });
  try {
    const state = inspect(made);
    const marker = join(made.directory, ".git/stock-copilot-merge-gate.json");
    writeFileSync(marker, JSON.stringify({ branch: "feature", ...state }));
    const guard = () =>
      spawnSync(
        "cargo",
        [
          "run",
          "--locked",
          "--quiet",
          "--manifest-path",
          join(repoRoot, "Cargo.toml"),
          "--package",
          "xtask",
          "--",
          "commit-guard",
          ".git/index",
        ],
        {
          cwd: made.directory,
          encoding: "utf8",
          env: {
            ...process.env,
            GIT_CONFIG_GLOBAL: "/dev/null",
            GIT_CONFIG_NOSYSTEM: "1",
            CARGO_BUILD_JOBS: "4",
            NEXTEST_TEST_THREADS: "4",
          },
        },
      );
    const prepared = guard();
    assert.equal(prepared.status, 0, prepared.stderr);
    const stored = JSON.parse(readFileSync(marker, "utf8"));
    assert.equal(stored.index, join(made.directory, ".git/index"));
    writeFileSync(marker, JSON.stringify({ ...stored, tree: "a".repeat(40) }));
    const rejected = guard();
    assert.notEqual(rejected.status, 0);
    made.git("merge", "--abort");
    const cleared = guard();
    assert.equal(cleared.status, 0, cleared.stderr);
    assert.equal(existsSync(marker), false);
  } finally {
    made.close();
  }
});
