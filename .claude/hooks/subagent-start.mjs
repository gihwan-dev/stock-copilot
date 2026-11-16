import {
  launchFacts,
  readAgentRules,
  snapshotFacts,
} from "./lib/agent-facts.mjs";
import { readInput, writeOutput } from "./lib/io.mjs";
import { HOOKS_PROJECT_ROOT, worktreesOf } from "./lib/project.mjs";
import { branchOf } from "./lib/git-facts.mjs";
import { reviewerTarget, snapshotOf } from "./lib/reviewer-guard.mjs";
import {
  KIND,
  claimRecord,
  listRecords,
  stateDirectory,
  writeRecord,
} from "./lib/state.mjs";
import { digestOf } from "./lib/brief.mjs";
import { ROLE } from "./lib/roles.mjs";
import {
  choosePending,
  firstPrompt,
  implementerStart,
  startKind,
  subagentToolUse,
  subagentTranscript,
} from "./lib/subagent-start.mjs";

const input = readInput();
const state = stateDirectory();
const kind = startKind(input);

const pause = (milliseconds) =>
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);

const promptWithRetry = (path, attempts) => {
  const prompt = firstPrompt(path);
  if (prompt !== undefined || attempts <= 1) {
    return prompt;
  }
  pause(200);
  return promptWithRetry(path, attempts - 1);
};

const context = (text) => ({
  hookSpecificOutput: {
    hookEventName: "SubagentStart",
    additionalContext: text,
  },
});

const claimFor = (transcribed) => {
  const mine = listRecords(state, KIND.pending).filter(
    (entry) => entry.sessionId === input.session_id,
  );
  const chosen = choosePending(
    mine,
    transcribed === undefined ? undefined : digestOf(transcribed),
    subagentToolUse(subagentTranscript(input)),
  );
  return chosen === undefined
    ? undefined
    : claimRecord(
        state,
        KIND.pending,
        `${chosen.sessionId}-${chosen.toolUseId}`,
        input.agent_id,
      )?.prompt;
};

if (kind === ROLE.implementer) {
  const rules = readAgentRules();
  const transcribed = promptWithRetry(subagentTranscript(input), 5);
  const claimed = claimFor(transcribed);
  const record = implementerStart(
    input,
    transcribed ?? claimed,
    launchFacts(HOOKS_PROJECT_ROOT, state, rules),
    rules,
  );
  writeRecord(state, KIND.implementer, input.agent_id, record);
  writeOutput(
    context(
      record.base === undefined
        ? `하네스: 기준 커밋을 기록하지 못했다 (${record.reason}). 종료 훅이 검사하지 않으므로 기록은 unchecked로 남는다.`
        : `하네스: 기준 커밋 ${record.base}을 기록했다. 종료하려 하면 하네스가 브랜치 ${record.branch}의 작업 사본(${record.worktree})에서 just base=${record.base} check를 돌린다.`,
    ),
  );
}

if (kind === ROLE.reviewer) {
  const worktrees = worktreesOf(HOOKS_PROJECT_ROOT);
  const target = reviewerTarget(
    promptWithRetry(subagentTranscript(input), 5),
    worktrees,
    { branchOf },
  );
  writeRecord(state, KIND.reviewer, input.agent_id, {
    kind: KIND.reviewer,
    agentId: input.agent_id,
    agentType: input.agent_type,
    sessionId: input.session_id,
    startedAt: new Date().toISOString(),
    status: target === undefined ? "unchecked" : "started",
    blocks: 0,
    worktree: target,
    reason:
      target === undefined
        ? "브리프의 리뷰 대상 작업 브랜치를 확인하지 못했다 (기준 커밋과 브랜치 필수, 등록된 대상 하나 필요)"
        : undefined,
    snapshot:
      target === undefined ? [] : snapshotOf(worktrees, target, snapshotFacts),
  });
}
