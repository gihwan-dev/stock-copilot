import { claudeSessionInput } from "./lib/session-role.mjs";
import { decideBash } from "./lib/bash-guard.mjs";
import { readInput, writeOutput } from "./lib/io.mjs";
import { loadProject } from "./lib/project.mjs";
import { preToolUseOutput } from "./lib/verdict.mjs";

writeOutput(
  preToolUseOutput(decideBash(claudeSessionInput(readInput()), loadProject())),
);
