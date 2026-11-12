import { readInput, writeOutput } from "./lib/io.mjs";
import { lintFeedback } from "./lib/post-edit.mjs";
import { HOOKS_PROJECT_ROOT, worktreesOf } from "./lib/project.mjs";

writeOutput(
  lintFeedback(readInput(), { worktrees: worktreesOf(HOOKS_PROJECT_ROOT) }),
);
