import { readAgentRules, snapshotFacts } from "./lib/agent-facts.mjs";
import { readInput, writeOutput } from "./lib/io.mjs";
import { HOOKS_PROJECT_ROOT, worktreesOf } from "./lib/project.mjs";
import { decideReviewerStop, snapshotOf } from "./lib/reviewer-guard.mjs";
import { KIND, readRecord, stateDirectory, writeRecord } from "./lib/state.mjs";

const input = readInput();
const state = stateDirectory();
const record = readRecord(state, KIND.reviewer, input.agent_id);
const result = decideReviewerStop(
  input,
  record,
  record?.worktree === undefined
    ? []
    : snapshotOf(
        worktreesOf(HOOKS_PROJECT_ROOT),
        record.worktree,
        snapshotFacts,
      ),
  readAgentRules(),
  new Date().toISOString(),
);

writeRecord(state, KIND.reviewer, input.agent_id, result.record);
writeOutput(result.output);
