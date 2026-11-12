import { execFileSync } from "node:child_process";
import { readFileSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseAllowlist } from "./allowlist.mjs";

export const HOOKS_PROJECT_ROOT = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
);

export const ALLOWLIST_FILE = "harness/agent-writable.txt";
export const LOCK_FILE = "harness/claude-hooks/lock.txt";
export const RULES_FILE = "harness/claude-hooks/rules.json";

export const LOCK = Object.freeze({ on: "on", off: "off" });

const GIT_REDIRECTS = [
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_INDEX_FILE",
  "GIT_COMMON_DIR",
  "GIT_OBJECT_DIRECTORY",
  "GIT_CONFIG_PARAMETERS",
  "GIT_CONFIG_COUNT",
];

export const gitEnvironment = () =>
  Object.fromEntries(
    Object.entries(process.env).filter(
      ([name]) => !GIT_REDIRECTS.includes(name),
    ),
  );

const readText = (root, relative) => {
  try {
    return { text: readFileSync(join(root, relative), "utf8") };
  } catch (error) {
    return { problem: `${relative}: ${error.message}` };
  }
};

export const readLock = (root) =>
  readText(root, LOCK_FILE).text === `${LOCK.off}\n` ? LOCK.off : LOCK.on;

export const readAllowlist = (root) => {
  const read = readText(root, ALLOWLIST_FILE);
  if (read.problem !== undefined) {
    return { problem: read.problem };
  }
  const parsed = parseAllowlist(read.text);
  return parsed.problem === undefined
    ? parsed
    : { problem: `${ALLOWLIST_FILE}: ${parsed.problem}` };
};

export const readRules = (root) =>
  JSON.parse(readFileSync(join(root, RULES_FILE), "utf8"));

const realOrSelf = (path) => {
  try {
    return realpathSync(path);
  } catch {
    return path;
  }
};

export const worktreesOf = (root) => {
  const listed = (() => {
    try {
      return execFileSync(
        "git",
        ["-C", root, "worktree", "list", "--porcelain", "-z"],
        {
          encoding: "utf8",
          env: gitEnvironment(),
          stdio: ["ignore", "pipe", "ignore"],
        },
      )
        .split("\0")
        .filter((field) => field.startsWith("worktree "))
        .map((field) => field.slice("worktree ".length));
    } catch {
      return [];
    }
  })();
  const roots = [root, ...listed].flatMap((path) => [path, realOrSelf(path)]);
  return [...new Set(roots)];
};

export const loadProject = (root = HOOKS_PROJECT_ROOT) => ({
  root,
  lock: readLock(root),
  allowlist: readAllowlist(root),
  rules: readRules(root),
  worktrees: worktreesOf(root),
});
