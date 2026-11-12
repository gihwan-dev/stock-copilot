import { createHash } from "node:crypto";
import { existsSync, lstatSync, readFileSync, readlinkSync } from "node:fs";
import { join } from "node:path";
import { runGit } from "./git-facts.mjs";

const blob = (content) =>
  createHash("sha1")
    .update(`blob ${content.length}\0`)
    .update(content)
    .digest("hex");

export const MISSING = "missing";

export const workingEntry = (root, path) => {
  try {
    const status = lstatSync(join(root, path));
    if (status.isSymbolicLink()) {
      return {
        kind: "link",
        hash: blob(Buffer.from(readlinkSync(join(root, path)))),
      };
    }
    if (!status.isFile()) {
      return { kind: "other", hash: "other" };
    }
    return {
      kind: (status.mode & 0o111) === 0 ? "file" : "executable",
      hash: blob(readFileSync(join(root, path))),
    };
  } catch {
    return { kind: MISSING, hash: MISSING };
  }
};

const rootIgnore = (root) =>
  existsSync(join(root, ".gitignore")) ? ["--exclude-from=.gitignore"] : [];

const nul = (text) => text.split("\0").filter((item) => item !== "");

const MODE_KIND = new Map([
  ["100644", "file"],
  ["100755", "executable"],
  ["120000", "link"],
]);

const LIMIT = 20;

const capped = (problems) =>
  problems.length > LIMIT
    ? [...problems.slice(0, LIMIT), `그 밖에 ${problems.length - LIMIT}건`]
    : problems;

export const treeProblems = (root) => {
  const flagged = runGit(root, ["ls-files", "-v", "-z"]);
  const staged = runGit(root, ["ls-files", "-s", "-z"]);
  const cached = runGit(root, [
    "diff-index",
    "--cached",
    "--name-only",
    "-z",
    "HEAD",
  ]);
  const untracked = runGit(root, [
    "ls-files",
    "--others",
    "-z",
    ...rootIgnore(root),
  ]);
  const failed = [flagged, staged, cached, untracked].filter(
    (result) => !result.ok,
  );
  if (failed.length > 0) {
    return [
      `git이 실패했다: ${failed.map((result) => result.stderr.trim()).join("; ")}`,
    ];
  }
  const marks = nul(flagged.stdout)
    .filter((entry) => /^[a-zS]/u.test(entry))
    .map((entry) => `assume-unchanged/skip-worktree 표시: ${entry.slice(2)}`);
  const indexProblems = nul(staged.stdout).flatMap((entry) => {
    const [meta, path] = entry.split("\t");
    const [mode, hash, stage] = meta.split(" ");
    if (stage !== "0") {
      return [`충돌 단계 항목: ${path}`];
    }
    const kind = MODE_KIND.get(mode);
    if (kind === undefined) {
      return [`서브모듈이나 모르는 항목: ${path}`];
    }
    const actual = workingEntry(root, path);
    if (actual.kind === MISSING) {
      return [`작업 트리에 없는 추적 파일: ${path}`];
    }
    return actual.hash === hash && actual.kind === kind
      ? []
      : [`스테이징하지 않은 변경: ${path}`];
  });
  return capped([
    ...marks,
    ...indexProblems,
    ...nul(cached.stdout).map((path) => `스테이징된 변경: ${path}`),
    ...nul(untracked.stdout).map((path) => `추적하지 않는 파일: ${path}`),
  ]);
};

export const contentSnapshot = (root) => {
  const head = runGit(root, ["rev-parse", "--verify", "--quiet", "HEAD"]);
  const index = runGit(root, ["ls-files", "-s", "-z"]);
  const files = runGit(root, [
    "ls-files",
    "--cached",
    "--others",
    "-z",
    ...rootIgnore(root),
  ]);
  if (!index.ok || !files.ok) {
    return { failed: `git이 실패했다: ${index.stderr}${files.stderr}`.trim() };
  }
  return {
    head: head.stdout.trim(),
    index: createHash("sha256").update(index.stdout).digest("hex"),
    files: Object.fromEntries(
      [...new Set(nul(files.stdout))].sort().map((path) => {
        const entry = workingEntry(root, path);
        return [path, `${entry.kind}:${entry.hash}`];
      }),
    ),
  };
};
