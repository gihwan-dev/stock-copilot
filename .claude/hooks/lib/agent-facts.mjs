import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { runHarnessCheck, checkPlaces } from "./check-sandbox.mjs";
import {
  branchOf,
  changedSince,
  commonDirectory,
  filesAt,
  headOf,
  isAncestor,
  refTip,
  resolveCommit,
} from "./git-facts.mjs";
import { contentSnapshot, treeProblems } from "./tree-hash.mjs";
import {
  HOOKS_PROJECT_ROOT,
  LOCK,
  readAllowlist,
  readLock,
  readRules,
  worktreesOf,
} from "./project.mjs";
import { KIND, STATUS, listRecords } from "./state.mjs";
import { firstPrompt } from "./subagent-start.mjs";

export const AGENT_RULES_FILE = "harness/claude-hooks/agent-rules.json";

export const readAgentRules = (root = HOOKS_PROJECT_ROOT) =>
  JSON.parse(readFileSync(join(root, AGENT_RULES_FILE), "utf8"));

export const passedTargets = (state) =>
  new Map(
    listRecords(state, KIND.implementer)
      .filter((record) => record.status === STATUS.passed && record.head)
      .map((record) => [record.head, record.agentId]),
  );

export const knownReportDigests = (state) =>
  new Set(
    listRecords(state, KIND.implementer).flatMap(
      (record) => record.reportDigests ?? [],
    ),
  );

export const originalWorktree = (root) => {
  const common = commonDirectory(root);
  return common === undefined ? root : dirname(common);
};

export const locateWorktree = (root, worktrees, branch) => {
  const original = originalWorktree(root);
  return branch === undefined
    ? undefined
    : worktrees.find((path) => path !== original && branchOf(path) === branch);
};

export const launchFacts = (root, state, rules) => {
  const worktrees = worktreesOf(root);
  return {
    resolveCommit: (text) => resolveCommit(root, text),
    baseTip: refTip(root, rules.baseRef),
    passedTargets: passedTargets(state),
    reportDigests: knownReportDigests(state),
    worktreeForBranch: (branch) => locateWorktree(root, worktrees, branch),
    now: () => new Date().toISOString(),
  };
};

export const FRAGMENT_FILE = "harness/sandbox/lock-fragment.json";

export const lockedDenies = (root, lock) => {
  if (lock === LOCK.off) {
    return [];
  }
  try {
    return JSON.parse(readFileSync(join(root, FRAGMENT_FILE), "utf8")).sandbox
      .filesystem.denyWrite;
  } catch {
    return ["./"];
  }
};

export const stopFacts = (root, state) => {
  const worktrees = worktreesOf(root);
  const rules = readRules(root);
  const lock = readLock(root);
  return {
    lock,
    allowlist: readAllowlist(root),
    locateWorktree: (branch) => locateWorktree(root, worktrees, branch),
    headOf,
    isAncestor,
    changedSince,
    treeProblems,
    existingAdrs: (worktree, base) =>
      filesAt(worktree, base, rules.paths.adr.directory),
    runCheck: (worktree, base) =>
      runHarnessCheck(
        checkPlaces({
          worktree,
          common: commonDirectory(worktree) ?? join(root, ".git"),
          projectRoot: root,
          state,
          locked: lockedDenies(root, lock),
        }),
        worktree,
        base,
      ),
    briefText: (input) =>
      typeof input.agent_transcript_path === "string"
        ? firstPrompt(input.agent_transcript_path)
        : undefined,
    now: () => new Date().toISOString(),
  };
};

export const snapshotFacts = { contentSnapshot };
