import { spawnSync } from "node:child_process";
import { gitEnvironment } from "./project.mjs";

const FULL_SHA = /^[0-9a-f]{40}$/u;

export const runGit = (cwd, args) => {
  const finished = spawnSync(
    "git",
    ["--no-replace-objects", "-c", "core.fsmonitor=false", "-C", cwd, ...args],
    {
      encoding: "utf8",
      env: { ...gitEnvironment(), GIT_GRAFT_FILE: "/dev/null" },
      stdio: ["ignore", "pipe", "pipe"],
      maxBuffer: 64 * 1024 * 1024,
    },
  );
  return {
    ok: finished.status === 0,
    stdout: finished.stdout ?? "",
    stderr: finished.stderr ?? "",
  };
};

const firstLine = (result) =>
  result.ok ? result.stdout.split("\n")[0].trim() : undefined;

export const resolveCommit = (cwd, text) => {
  const resolved = firstLine(
    runGit(cwd, [
      "rev-parse",
      "--verify",
      "--quiet",
      "--end-of-options",
      `${text}^{commit}`,
    ]),
  );
  return resolved !== undefined && FULL_SHA.test(resolved)
    ? resolved
    : undefined;
};

export const refTip = (cwd, ref) => resolveCommit(cwd, ref);

export const headOf = (cwd) => resolveCommit(cwd, "HEAD");

export const branchOf = (cwd) => {
  const name = firstLine(runGit(cwd, ["symbolic-ref", "--quiet", "HEAD"]));
  return name === undefined ? undefined : name.replace(/^refs\/heads\//u, "");
};

export const isAncestor = (cwd, ancestor, descendant) =>
  runGit(cwd, ["merge-base", "--is-ancestor", ancestor, descendant]).ok;

export const commonDirectory = (cwd) =>
  firstLine(
    runGit(cwd, ["rev-parse", "--path-format=absolute", "--git-common-dir"]),
  );

const nulList = (result) =>
  result.ok ? result.stdout.split("\0").filter((item) => item !== "") : [];

export const statusOf = (cwd) => {
  const result = runGit(cwd, ["status", "--porcelain=v1", "-uall", "-z"]);
  return result.ok ? nulList(result).sort() : undefined;
};

export const changedSince = (cwd, base) => [
  ...new Set([
    ...nulList(runGit(cwd, ["diff", "--name-only", "-z", base])),
    ...nulList(
      runGit(cwd, ["ls-files", "--others", "-z", "--exclude-from=.gitignore"]),
    ),
  ]),
];

export const filesAt = (cwd, commit, directory) =>
  runGit(cwd, ["ls-tree", "--name-only", commit, `${directory}/`])
    .stdout.split("\n")
    .filter((line) => line !== "");
