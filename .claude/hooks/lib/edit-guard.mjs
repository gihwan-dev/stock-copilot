import { fixedFindings, locate, roleFindings } from "./paths.mjs";
import { ROLE, roleOf } from "./roles.mjs";
import { decide } from "./verdict.mjs";

const targetOf = (input) => {
  const toolInput = input.tool_input ?? {};
  return toolInput.file_path ?? toolInput.notebook_path;
};

export const editFindings = (input, project) => {
  const role = roleOf(input);
  if (role === ROLE.reviewer || role === ROLE.unknown) {
    return [
      {
        rule: "role-without-edits",
        gated: false,
        message: `${input.agent_type ?? "이 서브에이전트"}는 파일을 편집하지 않는다 (docs/workflow.md 1장)`,
      },
    ];
  }
  const target = targetOf(input);
  const located = locate(target, project.worktrees);
  if (located.problem !== undefined) {
    return [
      {
        rule: "unsafe-path",
        gated: false,
        message: `${located.problem}: ${target}`,
      },
    ];
  }
  const fixed = fixedFindings(located.relative, project.rules);
  return fixed.length > 0
    ? fixed
    : roleFindings(located.relative, role, project.allowlist, project.rules);
};

export const decideEdit = (input, project) =>
  decide(editFindings(input, project), project.lock);
