import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { stateDirectory, writeRecord } from "../../.claude/hooks/lib/state.mjs";
import { homedir } from "node:os";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { sessionProject } from "./session.mjs";
import {
  codexPreTool,
  patchPaths,
  shellPatches,
  toolCommand,
  refusal,
} from "./adapter.mjs";
import {
  checkPlaces,
  runHarnessCheck,
  runInSandbox,
} from "../../.claude/hooks/lib/check-sandbox.mjs";
import { git } from "../../.claude/hooks/lib/project.mjs";
import { lintFeedback } from "../../.claude/hooks/lib/post-edit.mjs";

export const dispatchEvent = (
  input,
  session,
  runners = { check: runHarnessCheck, lint: runInSandbox },
  record = writeRecord,
) => {
  const { project, common: main, root, cwd } = session;
  const places = checkPlaces({
    worktree: root,
    common: main,
    projectRoot: project.root,
    state: join(homedir(), ".local/state/stock-copilot-harness"),
    locked:
      project.lock === "on"
        ? JSON.parse(
            readFileSync(
              join(project.root, "harness/sandbox/lock-fragment.json"),
              "utf8",
            ),
          ).sandbox.filesystem.denyWrite
        : [],
  });
  places.unreadable.push(
    `(subpath ${JSON.stringify(join(homedir(), ".codex"))})`,
  );
  let output;
  if (input.hook_event_name === "PreToolUse") {
    output = codexPreTool(input, project, cwd);
    if (typeof input.tool_use_id === "string")
      record(
        stateDirectory(),
        "codexaudit",
        createHash("sha256").update(input.tool_use_id).digest("hex"),
        {
          event: "PreToolUse",
          tool: input.tool_name,
          inputDigest: createHash("sha256")
            .update(JSON.stringify(input.tool_input))
            .digest("hex"),
          decision: output?.hookSpecificOutput?.permissionDecision ?? "allow",
        },
      );
  }
  if (
    input.hook_event_name === "PostToolUse" &&
    ["apply_patch", "Bash", "exec_command", "shell_command", "shell"].includes(
      input.tool_name,
    )
  ) {
    const patches =
      input.tool_name === "apply_patch"
        ? [input.tool_input.command]
        : shellPatches(toolCommand(input));
    const directory =
      input.tool_name === "apply_patch" ? cwd : (input.cwd ?? cwd);
    const failed = [...new Set(patches.flatMap(patchPaths))]
      .map((path) =>
        lintFeedback(
          { tool_input: { file_path: resolve(directory, path) } },
          project,
          (worktree, path) =>
            runners.lint(places, worktree, ["just", "lint-file", path]),
        ),
      )
      .filter((value) => value !== undefined);
    if (failed.length > 0)
      output = {
        decision: "block",
        reason: failed.map((value) => value.reason).join("\n"),
      };
  }
  if (input.hook_event_name === "Stop" && root !== project.root) {
    const base = git(root, ["merge-base", "refs/heads/master", "HEAD"])?.trim();
    if (base === undefined)
      output = {
        decision: "block",
        reason: "기준 커밋을 풀 수 없다. 막힘으로 보고하라",
      };
    else {
      const check = runners.check(places, root, base);
      if (typeof input.session_id === "string") {
        const head = git(root, ["rev-parse", "HEAD"])?.trim();
        const clean =
          git(root, ["status", "--porcelain=v1", "-uall"])?.trim() === "";
        const id =
          "codex-" +
          createHash("sha256").update(input.session_id).digest("hex");
        record(stateDirectory(), "implementer", id, {
          agentId: id,
          kind: "implementer",
          worktree: root,
          base,
          head,
          status: check.status === 0 && clean ? "passed" : "failing",
        });
      }

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
  return output;
};
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  let input;
  try {
    input = JSON.parse(readFileSync(0, "utf8"));
    const output = dispatchEvent(input, sessionProject());
    if (output !== undefined)
      process.stdout.write(JSON.stringify(output) + "\n");
  } catch (error) {
    const reason = "Codex 하네스 오류: " + error.message;
    process.stderr.write(reason + "\n");
    const event = process.argv[2] ?? input?.hook_event_name ?? "PreToolUse";
    process.stdout.write(
      JSON.stringify(
        event === "PreToolUse"
          ? refusal(reason)
          : { decision: "block", reason },
      ) + "\n",
    );
  }
}
