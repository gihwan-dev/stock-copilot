import { createHash } from "node:crypto";
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { git } from "../../.claude/hooks/lib/project.mjs";

export const HASHES_FILE = "harness/claude-hooks/hashes.json";

const NAMED = [
  "harness/claude-hooks/rules.json",
  "harness/agent-writable.txt",
  "harness/claude-hooks/lock.txt",
  "harness/claude-hooks/agent-rules.json",
  "harness/sandbox/lock-fragment.json",
];
export const HASH_DIRECTORIES = [
  ".claude/hooks",
  ".codex",
  "harness/codex-hooks",
  "harness/toolchain",
  "harness/codex-regression",
];
const trackedHooks = (root) => {
  const files = git(root, ["ls-files", "-z"]);
  if (files === undefined)
    throw new Error("해시 대상의 Git 추적 목록을 읽지 못했다");
  return files
    .split("\0")
    .filter(
      (path) =>
        path &&
        (NAMED.includes(path) ||
          HASH_DIRECTORIES.some((directory) =>
            path.startsWith(directory + "/"),
          )),
    );
};
export const currentHashes = (root) =>
  Object.fromEntries(
    trackedHooks(root)
      .filter((path) => existsSync(join(root, path)))
      .sort()
      .map((path) => [
        path,
        createHash("sha256")
          .update(readFileSync(join(root, path)))
          .digest("hex"),
      ]),
  );

export const hashProblems = (root, expected) => {
  const actual = currentHashes(root);
  const paths = [
    ...new Set([...Object.keys(actual), ...Object.keys(expected)]),
  ].sort();
  return paths
    .filter((path) => actual[path] !== expected[path])
    .map((path) =>
      actual[path] === undefined
        ? `${path}: 기대값에 있는 파일이 없다`
        : expected[path] === undefined
          ? `${path}: 기대값에 없는 파일이다`
          : `${path}: 내용이 기대값과 다르다`,
    );
};

const repoRoot = resolve(fileURLToPath(import.meta.url), "..", "..", "..");
export const HASHED = trackedHooks(repoRoot).filter(
  (path) => !path.startsWith(".claude/hooks/"),
);

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  if (process.argv[2] === "--write") {
    writeFileSync(
      join(repoRoot, HASHES_FILE),
      `${JSON.stringify(currentHashes(repoRoot), undefined, 2)}\n`,
    );
  }
  const problems = hashProblems(
    repoRoot,
    JSON.parse(readFileSync(join(repoRoot, HASHES_FILE), "utf8")),
  );
  problems.forEach((problem) => process.stdout.write(`FAIL ${problem}\n`));
  process.stdout.write(`claude-hooks hashes: ${problems.length} failure(s)\n`);
  process.exitCode = problems.length === 0 ? 0 : 1;
}
