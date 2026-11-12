import { lstatSync } from "node:fs";
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
  problems.forEach((problem) => process.stdout.write(`FAIL ${problem}\n`));
  process.stdout.write(`prelock: ${problems.length} failure(s)\n`);
  process.exitCode = problems.length === 0 ? 0 : 1;
}
