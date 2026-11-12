import { MODE, shapeProblem, writeFindings } from "./paths.mjs";
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
  const shape = shapeProblem(target);
  if (shape !== undefined) {
    return [
      {
        rule: "unsafe-path",
        gated: false,
        message: `${shape}: ${target}`,
      },
    ];
  }
  return writeFindings(target, role, project, MODE.edit);
};

export const decideEdit = (input, project) =>
  decide(editFindings(input, project), project.lock);
