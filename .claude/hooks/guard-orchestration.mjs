import { launchFacts, readAgentRules } from "./lib/agent-facts.mjs";
import { readInput, writeOutput } from "./lib/io.mjs";
import { orchestrationFindings } from "./lib/orchestration.mjs";
import { HOOKS_PROJECT_ROOT, LOCK } from "./lib/project.mjs";
import { stateDirectory } from "./lib/state.mjs";
import { decide, preToolUseOutput } from "./lib/verdict.mjs";
writeOutput(
  preToolUseOutput(
    decide(
      orchestrationFindings(
        readInput(),
        readAgentRules(),
        launchFacts(HOOKS_PROJECT_ROOT, stateDirectory(), readAgentRules()),
      ),
      LOCK.on,
    ),
  ),
);
