import { isAbsolute, resolve } from "node:path";
import { decideBash } from "../../.claude/hooks/lib/bash-guard.mjs";
import { editFindings } from "../../.claude/hooks/lib/edit-guard.mjs";
import { locate } from "../../.claude/hooks/lib/paths.mjs";
import { decide, preToolUseOutput } from "../../.claude/hooks/lib/verdict.mjs";

const refusal = (message) =>
  preToolUseOutput({
    decision: "deny",
    findings: [{ rule: "codex-input", message }],
  });

export const patchPaths = (patch) => {
  if (typeof patch !== "string") throw new Error("패치 문자열이 없다");
  const lines = patch.trim().split("\n");
  if (lines[0] !== "*** Begin Patch" || lines.at(-1) !== "*** End Patch")
    throw new Error("패치 경계가 잘못됐다");
  const paths = lines.flatMap((line) => {
    const match =
      /^\*\*\* (?:Add File|Update File|Delete File|Move to): (.+)$/u.exec(line);
    return match === null ? [] : [match[1]];
  });
  if (paths.length === 0) throw new Error("패치 대상이 없다");
  return [...new Set(paths)];
};

export const actorInput = (input, project) => {
  const cwd = input.cwd;
  if (typeof cwd !== "string" || !isAbsolute(cwd))
    throw new Error("현재 폴더가 없다");
  const located = locate(resolve(cwd, ".codex-role-probe"), project.worktrees);
  if (located.root === undefined) throw new Error("등록된 작업 사본 밖이다");
  if (input.agent_id !== undefined || input.agent_type !== undefined)
    throw new Error("Codex 내부 서브에이전트는 쓰지 않는다");
  return located.root === project.root
    ? {}
    : { agent_type: "implementer", agent_id: "codex-worktree" };
};

export const codexPreTool = (input, project) => {
  try {
    const actor = actorInput(input, project);
    if (input.tool_name === "Bash") {
      if (typeof input.tool_input?.command !== "string")
        return refusal("셸 명령이 없다");
      return preToolUseOutput(
        decideBash(
          {
            ...input,
            ...actor,
            tool_input: { ...input.tool_input, cwd: input.cwd },
          },
          project,
        ),
      );
    }
    if (input.tool_name === "apply_patch") {
      const findings = patchPaths(input.tool_input?.command).flatMap((path) =>
        editFindings(
          {
            ...actor,
            tool_input: {
              file_path: isAbsolute(path) ? path : `${input.cwd}/${path}`,
            },
          },
          project,
        ),
      );
      return preToolUseOutput(decide(findings, project.lock));
    }
    if (
      ["spawn_agent", "Agent", "send_input", "resume_agent"].includes(
        input.tool_name,
      )
    )
      return refusal(
        "구현과 비판은 T3가 새 Codex 작업 사본에서 시작한다. 내부 위임과 재개는 쓰지 않는다",
      );
    return undefined;
  } catch (error) {
    return refusal(error.message);
  }
};
