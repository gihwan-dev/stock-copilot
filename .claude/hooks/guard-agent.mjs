import { launchFacts, readAgentRules } from "./lib/agent-facts.mjs";
import { agentFindings } from "./lib/agent-guard.mjs";
import { digestOf } from "./lib/brief.mjs";
import { readInput, writeOutput } from "./lib/io.mjs";
import { HOOKS_PROJECT_ROOT, LOCK } from "./lib/project.mjs";
import { KIND, isSafeId, stateDirectory, writeRecord } from "./lib/state.mjs";
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
  input.tool_input?.subagent_type === "implementer" &&
  isSafeId(input.session_id) &&
  isSafeId(input.tool_use_id) &&
  typeof prompt === "string"
) {
  writeRecord(state, KIND.pending, `${input.session_id}-${input.tool_use_id}`, {
    sessionId: input.session_id,
    toolUseId: input.tool_use_id,
    digest: digestOf(prompt),
    prompt,
    at: new Date().toISOString(),
  });
}
writeOutput(preToolUseOutput(verdict));
