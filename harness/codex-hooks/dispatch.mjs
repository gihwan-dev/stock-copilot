import { homedir } from "node:os";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { codexPreTool, patchPaths } from "./adapter.mjs";
import {
  checkPlaces,
  runHarnessCheck,
  runInSandbox,
} from "../../.claude/hooks/lib/check-sandbox.mjs";
import { loadProject, git } from "../../.claude/hooks/lib/project.mjs";
import { lintFeedback } from "../../.claude/hooks/lib/post-edit.mjs";

const mainHook = () => {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
  const main = git(root, [
    "rev-parse",
    "--path-format=absolute",
    "--git-common-dir",
  ])?.trim();
  const project = loadProject(root);
  project.root = main === undefined ? root : dirname(main);
  const input = JSON.parse(readFileSync(0, "utf8"));
  const places = checkPlaces({
    worktree: input.cwd,
    common: main,
    projectRoot: project.root,
    state: join(homedir(), ".local/state/stock-copilot-harness"),
    locked:
      project.lock === "on"
        ? JSON.parse(
            readFileSync(
              join(root, "harness/sandbox/lock-fragment.json"),
              "utf8",
            ),
          ).sandbox.filesystem.denyWrite
        : [],
  });
  places.unreadable.push(
    `(subpath ${JSON.stringify(join(homedir(), ".codex"))})`,
  );
  let output;
  if (input.hook_event_name === "PreToolUse")
    output = codexPreTool(input, project);
  if (
    input.hook_event_name === "PostToolUse" &&
    input.tool_name === "apply_patch"
  ) {
    const failed = patchPaths(input.tool_input.command)
      .map((path) =>
        lintFeedback(
          { tool_input: { file_path: resolve(input.cwd, path) } },
          project,
          (worktree, path) =>
            runInSandbox(places, worktree, ["just", "lint-file", path]),
        ),
      )
      .filter((value) => value !== undefined);
    if (failed.length > 0)
      output = {
        decision: "block",
        reason: failed.map((value) => value.reason).join("\n"),
      };
  }
  if (input.hook_event_name === "Stop" && input.cwd !== project.root) {
    const base = git(input.cwd, [
      "merge-base",
      "refs/heads/master",
      "HEAD",
    ])?.trim();
    if (base === undefined)
      output = {
        decision: "block",
        reason: "기준 커밋을 풀 수 없다. 막힘으로 보고하라",
      };
    else {
      const check = runHarnessCheck(places, input.cwd, base);
      if (check.status !== 0) {
        const blocked =
          input.stop_hook_active === true &&
          input.last_assistant_message?.startsWith("막힘:");
        output = blocked
          ? { systemMessage: "전체 검사 실패: 완료가 아니다" }
          : {
              decision: "block",
              reason:
                `just check 실패. 고칠 수 없으면 막힘으로 보고하라.\n${check.output}`.slice(
                  -8000,
                ),
            };
      }
    }
  }
  if (output !== undefined) process.stdout.write(`${JSON.stringify(output)}\n`);
};
try {
  mainHook();
} catch (error) {
  process.stdout.write(
    `${JSON.stringify({ decision: "block", reason: `Codex 하네스 오류: ${error.message}` })}\n`,
  );
  process.exitCode = 2;
}
