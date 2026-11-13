import { lstatSync, readFileSync, existsSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  isWritable,
  patternMatches,
} from "../../.claude/hooks/lib/allowlist.mjs";
import { resolveCommit, runGit } from "../../.claude/hooks/lib/git-facts.mjs";
import { treeProblems } from "../../.claude/hooks/lib/tree-hash.mjs";
import { readAllowlistFile, readPolicy, trackedFiles } from "./fragment.mjs";

export const LOCKED_TREES = [
  "harness",
  ".claude",
  ".codex",
  "justfile",
  "crates/xtask",
  "lefthook.yml",
];

const linkCount = (root, path) => {
  try {
    return lstatSync(join(root, path)).nlink;
  } catch {
    return 1;
  }
};

export const prelockProblems = (root, reviewed, allowlist, policy) => {
  const commit = resolveCommit(root, reviewed);
  if (commit === undefined) {
    return [`검토한 커밋 ${reviewed}을 풀 수 없다`];
  }
  const diff = runGit(root, [
    "diff",
    "--name-only",
    commit,
    "--",
    ...LOCKED_TREES,
  ]);
  const protectedPath = (path) =>
    !isWritable(allowlist, path) ||
    policy.lock.trackedProtected.some((pattern) =>
      patternMatches(pattern, path),
    );
  return [
    ...treeProblems(root).map((problem) => `작업 트리: ${problem}`),
    ...(diff.ok
      ? diff.stdout
          .split("\n")
          .filter((line) => line !== "")
          .map((path) => `검토한 커밋과 다르다: ${path}`)
      : [`git diff가 실패했다: ${diff.stderr.trim()}`]),
    ...trackedFiles(root)
      .filter(protectedPath)
      .filter((path) => linkCount(root, path) > 1)
      .map((path) => `하드 링크가 있는 보호 파일: ${path}`),
  ];
};

export const codexUserProfileProblems = (text) =>
  text.includes("stock-copilot-harness")
    ? [
        "Codex 사용자 설정에 하네스 프로필이 남아 있다. 해당 프로필과 선택 값을 지우고 다시 검사하라",
      ]
    : [];
const userProfileProblems = (path) => {
  try {
    return existsSync(path)
      ? codexUserProfileProblems(readFileSync(path, "utf8"))
      : [];
  } catch {
    return ["Codex 사용자 설정을 확인하지 못했다. 사용자가 직접 확인하라"];
  }
};
const repoRoot = resolve(fileURLToPath(import.meta.url), "..", "..", "..");

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const reviewed = process.argv[2];
  const problems =
    reviewed === undefined
      ? ["사용법: node harness/sandbox/prelock.mjs <검토한 커밋>"]
      : prelockProblems(
          repoRoot,
          reviewed,
          readAllowlistFile(repoRoot),
          readPolicy(repoRoot),
        );
  if (process.argv.includes("--codex-user-config")) {
    const index = process.argv.indexOf("--codex-user-config");
    const path =
      process.argv[index + 1] ??
      join(process.env.CODEX_HOME ?? join(homedir(), ".codex"), "config.toml");
    problems.push(...userProfileProblems(path));
  }
  problems.forEach((problem) => process.stdout.write(`FAIL ${problem}\n`));
  process.stdout.write(`prelock: ${problems.length} failure(s)\n`);
  process.exitCode = problems.length === 0 ? 0 : 1;
}
