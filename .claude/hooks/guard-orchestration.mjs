import { launchFacts, readAgentRules } from "./lib/agent-facts.mjs";
import { readInput, writeOutput } from "./lib/io.mjs";
import { orchestrationFindings, threadResult } from "./lib/orchestration.mjs";
import { HOOKS_PROJECT_ROOT, LOCK } from "./lib/project.mjs";
import { headOf, isAncestor } from "./lib/git-facts.mjs";
import { listRecords, stateDirectory, writeRecord } from "./lib/state.mjs";
import { decide, preToolUseOutput } from "./lib/verdict.mjs";
import { parseBrief } from "./lib/brief.mjs";
const input = readInput();
const state = stateDirectory();
const rules = readAgentRules();
const facts = launchFacts(HOOKS_PROJECT_ROOT, state, rules);
facts.headOf = headOf;
facts.isAncestor = (a, b) => isAncestor(HOOKS_PROJECT_ROOT, a, b);
facts.t3Threads = new Map(
  listRecords(state, "t3thread")
    .filter((record) => record.owner === input.session_id)
    .map((record) => [record.threadId, record]),
);
if (input.hook_event_name === "PostToolUse") {
  const result = threadResult(input.tool_response);
  const threadId = result?.threadId ?? result?.childThreadId;
  const text = input.tool_input?.message ?? input.tool_input?.task;
  const brief = typeof text === "string" ? parseBrief(text) : {};
  if (typeof threadId === "string" && brief.branch !== undefined)
    writeRecord(state, "t3thread", threadId, {
      owner: input.session_id,
      threadId,
      branch: brief.branch,
    });
} else
  writeOutput(
    preToolUseOutput(
      decide(orchestrationFindings(input, rules, facts), LOCK.on),
    ),
  );
