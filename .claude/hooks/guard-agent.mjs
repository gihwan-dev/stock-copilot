import { launchFacts, readAgentRules } from "./lib/agent-facts.mjs";
import { agentFindings } from "./lib/agent-guard.mjs";
import { digestOf } from "./lib/brief.mjs";
import { readInput, writeOutput } from "./lib/io.mjs";
import { HOOKS_PROJECT_ROOT, LOCK } from "./lib/project.mjs";
import {
  KIND,
  isSafeId,
  readRecord,
  stateDirectory,
  writeRecord,
} from "./lib/state.mjs";
import { withPending } from "./lib/subagent-start.mjs";
import { decide, preToolUseOutput } from "./lib/verdict.mjs";

const input = readInput();
const rules = readAgentRules();
const state = stateDirectory();
const verdict = decide(
  agentFindings(input, rules, launchFacts(HOOKS_PROJECT_ROOT, state, rules)),
  LOCK.on,
);
const prompt = input.tool_input?.prompt;

if (
  verdict.findings.length === 0 &&
  isSafeId(input.session_id) &&
  typeof prompt === "string"
) {
  writeRecord(
    state,
    KIND.pending,
    input.session_id,
    withPending(readRecord(state, KIND.pending, input.session_id), {
      type: input.tool_input.subagent_type,
      digest: digestOf(prompt),
      prompt,
      toolUseId: input.tool_use_id,
      at: new Date().toISOString(),
    }),
  );
}
writeOutput(preToolUseOutput(verdict));
