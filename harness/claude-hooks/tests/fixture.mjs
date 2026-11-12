import { execFileSync, spawnSync } from "node:child_process";
import {
  linkSync,
  mkdirSync,
  rmSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  LOCK,
  gitEnvironment,
  readAllowlist,
  readRules,
  trackedAdrs,
  worktreesOf,
} from "../../../.claude/hooks/lib/project.mjs";

export const repoRoot = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
  "..",
);

export const realRules = readRules(repoRoot);
export const realAllowlist = readAllowlist(repoRoot);

export const readJson = (relative) =>
  JSON.parse(readFileSync(join(repoRoot, relative), "utf8"));

const git = (cwd, ...args) =>
  execFileSync(
    "git",
    [
      "-c",
      "user.name=hooks",
      "-c",
      "user.email=hooks@example.invalid",
      "-c",
      "commit.gpgsign=false",
      "-c",
      "init.defaultBranch=master",
      ...args,
    ],
    { cwd, env: gitEnvironment(), stdio: "pipe" },
  );

const FILES = [
  "justfile",
  "Cargo.toml",
  "crates/domain/src/lib.rs",
  "docs/notes.md",
  "docs/harness.md",
  "harness/agent-writable.txt",
  "harness/claude-hooks/lock.txt",
  "harness/linked-source.txt",
  "docs/adr/0001-first.md",
  "docs/adr/0002-second.md",
  "docs/\uD55C\uAE00.md",
];

export const createFixture = () => {
  const base = realpathSync(mkdtempSync(join(tmpdir(), "claude-hooks-")));
  const root = join(base, "repo");
  for (const file of FILES) {
    mkdirSync(dirname(join(root, file)), { recursive: true });
    writeFileSync(join(root, file), "fixture\n");
  }
  mkdirSync(join(base, "outside"), { recursive: true });
  git(root, "init", "--quiet");
  git(root, "add", "--all");
  git(root, "commit", "--quiet", "-m", "fixture");
  git(root, "worktree", "add", "--quiet", "-b", "side", join(base, "worktree"));
  git(root, "config", "alias.ci", "commit -n");
  git(root, "config", "alias.sneak", "!git commit --no-verify");
  symlinkSync("../../../justfile", join(root, "crates/domain/src/link.rs"));
  symlinkSync("crates", join(root, "linked"));
  linkSync(
    join(root, "harness/linked-source.txt"),
    join(root, "docs/linked.txt"),
  );
  git(root, "worktree", "add", "--quiet", "-b", "swap", join(base, "wx"));
  rmSync(join(base, "wx"), { recursive: true, force: true });
  symlinkSync(join(root, "harness"), join(base, "wx"));
  return {
    base,
    root,
    worktree: join(base, "worktree"),
    outside: join(base, "outside"),
  };
};

const worktreeCache = new Map();

const cachedWorktrees = (root) => {
  if (!worktreeCache.has(root)) {
    worktreeCache.set(root, worktreesOf(root));
  }
  return worktreeCache.get(root);
};

const adrCache = new Map();

const cachedAdrs = (root, rules) => {
  const key = `${root}\0${JSON.stringify(rules.paths.adr)}`;
  if (!adrCache.has(key)) {
    adrCache.set(key, trackedAdrs(root, rules));
  }
  return adrCache.get(key);
};

export const projectFor = (fixture, overrides = {}) => ({
  root: fixture.root,
  lock: overrides.lock ?? LOCK.off,
  allowlist: overrides.allowlist ?? realAllowlist,
  rules: overrides.rules ?? realRules,
  worktrees: cachedWorktrees(fixture.root),
  adrs: cachedAdrs(fixture.root, overrides.rules ?? realRules),
});

export const placeholders = (fixture) => (text) =>
  text
    .replaceAll("{root}", fixture.root)
    .replaceAll("{worktree}", fixture.worktree)
    .replaceAll("{outside}", fixture.outside)
    .replaceAll("{base}", fixture.base)
    .replaceAll("{home}", homedir())
    .replaceAll("{tmp}", realpathSync(tmpdir()));

export const roleInput = (role) => {
  if (role === undefined || role === "main") {
    return {};
  }
  if (role === "agent-id-only") {
    return { agent_id: "a1" };
  }
  return { agent_type: role, agent_id: "a1" };
};

export const runHook = (script, input) => {
  const finished = spawnSync(
    process.execPath,
    [join(repoRoot, ".claude/hooks", script)],
    {
      input: typeof input === "string" ? input : JSON.stringify(input),
      encoding: "utf8",
      env: { ...process.env, CLAUDE_PROJECT_DIR: repoRoot },
      timeout: 300000,
    },
  );
  const text = finished.stdout.trim();
  return {
    status: finished.status,
    stdout: text,
    output: text === "" ? undefined : JSON.parse(text),
    stderr: finished.stderr,
  };
};

export const ruleSet = (verdict) =>
  [...new Set(verdict.findings.map((finding) => finding.rule))].sort();

export const withoutIndex = (list, index) => [
  ...list.slice(0, index),
  ...list.slice(index + 1),
];

const MAP_PATHS = ["git.shortValues", "interpreters"];

export const dataVariants = (value, path = []) => {
  if (Array.isArray(value)) {
    return value.flatMap((element, index) => [
      {
        name: `${[...path, index].join(".")}=${JSON.stringify(element)}`,
        path,
        replace: withoutIndex(value, index),
      },
      ...dataVariants(element, [...path, index]),
    ]);
  }
  if (typeof value === "string") {
    return path.includes("shortValues")
      ? [...value].map((character, index) => ({
          name: `${path.join(".")} without ${character}`,
          path,
          replace: value.slice(0, index) + value.slice(index + 1),
        }))
      : [];
  }
  if (value === null || typeof value !== "object") {
    return [];
  }
  const keys = MAP_PATHS.includes(path.join("."))
    ? Object.keys(value).map((key) => ({
        name: `${path.join(".")} without ${key}`,
        path,
        replace: Object.fromEntries(
          Object.entries(value).filter(([other]) => other !== key),
        ),
      }))
    : [];
  return [
    ...keys,
    ...Object.entries(value).flatMap(([key, child]) =>
      dataVariants(child, [...path, key]),
    ),
  ];
};

export const applyVariant = (document, variant) => {
  const copy = structuredClone(document);
  if (variant.path.length === 0) {
    return variant.replace;
  }
  const parent = variant.path
    .slice(0, -1)
    .reduce((node, key) => node[key], copy);
  parent[variant.path.at(-1)] = variant.replace;
  return copy;
};
