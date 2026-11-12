import { decideEdit } from "./lib/edit-guard.mjs";
import { readInput, writeOutput } from "./lib/io.mjs";
import { loadProject } from "./lib/project.mjs";
import { preToolUseOutput } from "./lib/verdict.mjs";

writeOutput(preToolUseOutput(decideEdit(readInput(), loadProject())));
