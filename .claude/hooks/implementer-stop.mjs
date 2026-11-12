import { readAgentRules, stopFacts } from "./lib/agent-facts.mjs";
import { decideImplementerStop } from "./lib/implementer-stop.mjs";
import { readInput, writeOutput } from "./lib/io.mjs";
import { HOOKS_PROJECT_ROOT } from "./lib/project.mjs";
import { KIND, readRecord, stateDirectory, writeRecord } from "./lib/state.mjs";

const input = readInput();
const state = stateDirectory();
const result = decideImplementerStop(
  input,
  readRecord(state, KIND.implementer, input.agent_id),
  stopFacts(HOOKS_PROJECT_ROOT, state),
  readAgentRules(),
);

writeRecord(state, KIND.implementer, input.agent_id, result.record);
writeOutput(result.output);
