import { execFileSync } from "node:child_process";
import { lstatSync, readFileSync, realpathSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseAllowlist } from "./allowlist.mjs";

const realOrSelf = (path) => {
  try {
    return realpathSync.native(path);
  } catch {
    return path;
  }
};

export const HOOKS_PROJECT_ROOT = realOrSelf(
  resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", ".."),
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

export const git = (cwd, args) => {
  try {
    return execFileSync("git", ["-C", cwd, ...args], {
      encoding: "utf8",
      env: gitEnvironment(),
      stdio: ["ignore", "pipe", "ignore"],
    });
  } catch {
    return undefined;
  }
};

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

const isLink = (path) => {
  try {
    return lstatSync(path).isSymbolicLink();
  } catch {
    return true;
  }
};

const readTrimmed = (path) => {
  try {
    return readFileSync(path, "utf8").trim();
  } catch {
    return undefined;
  }
};

const linkedWorktreeIsGenuine = (path, common) => {
  if (isLink(path) || realOrSelf(path) !== path) {
    return false;
  }
  const pointer = readTrimmed(join(path, ".git"));
  if (pointer === undefined || !pointer.startsWith("gitdir: ")) {
    return false;
  }
  const administrative = realOrSelf(pointer.slice("gitdir: ".length));
  if (dirname(administrative) !== join(common, "worktrees")) {
    return false;
  }
  const back = readTrimmed(join(administrative, "gitdir"));
  return back !== undefined && realOrSelf(back) === join(path, ".git");
};

export const worktreesOf = (root) => {
  const commonText = git(root, [
    "rev-parse",
    "--path-format=absolute",
    "--git-common-dir",
  ]);
  if (commonText === undefined) {
    return [root];
  }
  const common = realOrSelf(commonText.trim());
  const listed = (git(root, ["worktree", "list", "--porcelain", "-z"]) ?? "")
    .split("\0")
    .filter((field) => field.startsWith("worktree "))
    .map((field) => field.slice("worktree ".length));
  const main = dirname(common);
  const genuine = listed.filter((path) =>
    path === main
      ? !isLink(path) && realOrSelf(path) === path
      : linkedWorktreeIsGenuine(path, common),
  );
  return [...new Set([root, ...genuine])];
};

export const trackedAdrs = (root, rules) =>
  (
    git(root, [
      "ls-tree",
      "--name-only",
      rules.paths.adr.reference,
      `${rules.paths.adr.directory}/`,
    ]) ?? ""
  )
    .split("\n")
    .filter((line) => line !== "");

export const loadProject = (root = HOOKS_PROJECT_ROOT) => {
  const rules = readRules(root);
  return {
    root,
    lock: readLock(root),
    allowlist: readAllowlist(root),
    rules,
    worktrees: worktreesOf(root),
    adrs: trackedAdrs(root, rules),
  };
};
