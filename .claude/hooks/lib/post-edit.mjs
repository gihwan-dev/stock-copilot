import { spawnSync } from "node:child_process";
import { locate } from "./paths.mjs";

const OUTPUT_LIMIT = 8000;
const LINT_TIMEOUT_MS = 280000;

export const runJustLintFile = (root, path) => {
  const finished = spawnSync("just", ["lint-file", path], {
    cwd: root,
    encoding: "utf8",
    timeout: LINT_TIMEOUT_MS,
    stdio: ["ignore", "pipe", "pipe"],
  });
  return {
    status: finished.status,
    output: `${finished.stdout ?? ""}${finished.stderr ?? ""}${finished.error === undefined ? "" : finished.error.message}`,
  };
};

export const lintFeedback = (input, project, run = runJustLintFile) => {
  const path = (input.tool_input ?? {}).file_path;
  const located = locate(path, project.worktrees);
  if (located.relative === undefined) {
    return undefined;
  }
  const result = run(located.root, path);
  if (result.status === 0) {
    return undefined;
  }
  return {
    decision: "block",
    reason: `just lint-file ${located.relative} 실패 (docs/harness.md 1.1). 검사를 끄지 말고 고쳐라.\n${result.output.slice(-OUTPUT_LIMIT)}`,
  };
};
