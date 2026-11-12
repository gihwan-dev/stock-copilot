import { readAgentRules } from "./lib/agent-facts.mjs";
import { agentResult } from "./lib/agent-result.mjs";
import { headOf } from "./lib/git-facts.mjs";
import { readInput, writeOutput } from "./lib/io.mjs";
import { KIND, readRecord, stateDirectory, writeRecord } from "./lib/state.mjs";

const input = readInput();
const state = stateDirectory();
const kindOf = (type) =>
  type === "implementer" ? KIND.implementer : KIND.reviewer;
const result = agentResult(
  input,
  (kind, id) => readRecord(state, kind, id),
  readAgentRules(),
  headOf,
);

if (result.record !== undefined) {
  writeRecord(
    state,
    kindOf(input.tool_input?.subagent_type),
    result.record.agentId,
    result.record,
  );
}
writeOutput(result.output);
