import { readFileSync } from "node:fs";
import { join } from "node:path";
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
  statusOf,
} from "./git-facts.mjs";
import {
  HOOKS_PROJECT_ROOT,
  readAllowlist,
  readLock,
  readRules,
  worktreesOf,
} from "./project.mjs";
import { digestOf } from "./brief.mjs";
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

export const knownReportLines = (state) => [
  ...new Set(
    listRecords(state, KIND.implementer).flatMap(
      (record) => record.reportLines ?? [],
    ),
  ),
];

export const launchFacts = (root, state, rules) => ({
  resolveCommit: (text) => resolveCommit(root, text),
  baseTip: refTip(root, rules.baseRef),
  passedTargets: passedTargets(state),
  reportLines: knownReportLines(state),
  now: () => new Date().toISOString(),
});

export const locateWorktree = (root, worktrees, branch) => {
  if (branch === undefined) {
    return root;
  }
  return worktrees.find((path) => branchOf(path) === branch);
};

export const stopFacts = (root, state) => {
  const worktrees = worktreesOf(root);
  const rules = readRules(root);
  return {
    lock: readLock(root),
    allowlist: readAllowlist(root),
    locateWorktree: (branch) => locateWorktree(root, worktrees, branch),
    headOf,
    isAncestor,
    changedSince,
    existingAdrs: (worktree, base) =>
      filesAt(worktree, base, rules.paths.adr.directory),
    runCheck: (worktree, base) =>
      runHarnessCheck(
        checkPlaces({
          worktree,
          common: commonDirectory(worktree) ?? join(root, ".git"),
          projectRoot: root,
          state,
        }),
        worktree,
        base,
      ),
    briefDigest: (input) => {
      const prompt =
        typeof input.agent_transcript_path === "string"
          ? firstPrompt(input.agent_transcript_path)
          : undefined;
      return prompt === undefined ? undefined : digestOf(prompt);
    },
    now: () => new Date().toISOString(),
  };
};

export const snapshotFacts = { headOf, statusOf };
