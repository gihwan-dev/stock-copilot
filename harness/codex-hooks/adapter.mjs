import { realpathSync } from "node:fs";
import { isAbsolute, resolve, basename } from "node:path";
import { decideBash } from "../../.claude/hooks/lib/bash-guard.mjs";
import { editFindings } from "../../.claude/hooks/lib/edit-guard.mjs";
import { locate } from "../../.claude/hooks/lib/paths.mjs";
import { parseScript } from "../../.claude/hooks/lib/shell.mjs";
import { decide, preToolUseOutput } from "../../.claude/hooks/lib/verdict.mjs";

export const refusal = (message) =>
  preToolUseOutput({
    decision: "deny",
    findings: [{ rule: "codex-input", message }],
  });
const asciiTrim = (line) => line.replace(/^[ \t]+|[ \t]+$/gu, "");
const forbiddenWhitespace = /[\p{Cc}\p{Zs}\p{Zl}\p{Zp}\uFEFF]/u;
export const patchPaths = (patch) => {
  if (typeof patch !== "string") throw new Error("패치 문자열이 없다");
  let lines = patch.replaceAll("\r\n", "\n").split("\n");
  while (lines[0] !== undefined && asciiTrim(lines[0]) === "") lines.shift();
  while (lines.at(-1) !== undefined && asciiTrim(lines.at(-1)) === "")
    lines.pop();
  if (
    ["<<EOF", "<<\u0027EOF\u0027", '<<"EOF"'].includes(
      asciiTrim(lines[0] ?? ""),
    ) &&
    asciiTrim(lines.at(-1) ?? "") === "EOF"
  )
    lines = lines.slice(1, -1);
  if (
    asciiTrim(lines[0] ?? "") !== "*** Begin Patch" ||
    asciiTrim(lines.at(-1) ?? "") !== "*** End Patch"
  )
    throw new Error("패치 경계가 잘못됐다");
  const paths = [];
  for (const [index, raw] of lines.entries()) {
    const line = asciiTrim(raw);
    if (
      (index === 0 && line === "*** Begin Patch") ||
      (index === lines.length - 1 && line === "*** End Patch") ||
      line === "*** End of File"
    )
      continue;
    const match =
      /^[ \t]*\*\*\* (?:Add File|Update File|Delete File|Move to): (.+)$/u.exec(
        raw,
      );
    if (match !== null) {
      if (
        [...match[1]].some(
          (character) =>
            character === " " ||
            character === "\t" ||
            forbiddenWhitespace.test(character),
        )
      )
        throw new Error("패치 경로에 공백이나 제어 문자가 있다");
      paths.push(match[1]);
      continue;
    }
    if (/^(?:[+-]|@@)/u.test(raw)) continue;
    if (line.includes("***")) throw new Error("엄격한 패치 머리줄이 아니다");
    const prefix = /^[ \t]*([^\x20-\x7e\t])/u.exec(raw);
    if (prefix !== null && forbiddenWhitespace.test(prefix[1]))
      throw new Error("머리줄 후보에 비 ASCII 공백이나 제어 문자가 있다");
    if (raw.startsWith(" ")) continue;
    throw new Error("패치 본문 줄을 분류하지 못했다");
  }
  if (paths.length === 0) throw new Error("패치 대상이 없다");
  return [...new Set(paths)];
};
export const actorInput = (input, project, sessionCwd = process.cwd()) => {
  const located = locate(
    resolve(sessionCwd, ".codex-role-probe"),
    project.worktrees,
  );
  if (located.root === undefined || located.problem !== undefined)
    throw new Error("등록된 세션 작업 사본이 아니다");
  if (input.agent_id !== undefined || input.agent_type !== undefined)
    throw new Error("Codex 내부 서브에이전트는 쓰지 않는다");
  return {
    root: located.root,
    actor:
      located.root === project.root
        ? {}
        : { agent_type: "implementer", agent_id: "codex-worktree" },
  };
};
const patchFindings = (patch, cwd, actor, project, owned) =>
  patchPaths(patch).flatMap((path) => {
    const absolute = resolve(cwd, path);
    if (!absolute.startsWith(`${owned}/`))
      throw new Error("세션 작업 사본 밖의 편집이다");
    return editFindings(
      { ...actor, tool_input: { file_path: absolute } },
      project,
    );
  });
export const shellPatches = (command) => {
  const parsed = parseScript(command);
  const calls = parsed.commands.filter((item) =>
    item.words.some((word) =>
      ["apply_patch", "applypatch"].includes(basename(word.text).toLowerCase()),
    ),
  );
  if (!calls.length && !/\bapply_?patch\b/iu.test(command)) return [];
  if (
    parsed.problems.length ||
    parsed.commands.length !== 1 ||
    calls.length !== 1 ||
    parsed.substitutions.length ||
    calls[0].words.length !== 1 ||
    calls[0].words[0].dynamic
  )
    throw new Error("패치 셸 호출을 정적으로 해석하지 못했다");
  return calls.map((call) => {
    const inputs = call.inputs;
    if (
      inputs.length !== 1 ||
      inputs[0].dynamic ||
      inputs[0].expand ||
      inputs[0].kind !== "heredoc" ||
      call.piped ||
      call.redirects.length
    )
      throw new Error("패치는 인용한 heredoc 본문 하나로만 실행한다");
    return inputs[0].text;
  });
};
export const toolCommand = (input) => {
  const value = input.tool_input?.command ?? input.tool_input?.cmd;
  const command = Array.isArray(value)
    ? value
        .map(
          (word) =>
            `\u0027${word.replaceAll("\u0027", "\u0027\\\u0027\u0027")}\u0027`,
        )
        .join(" ")
    : value;
  if (typeof command !== "string") throw new Error("셸 명령이 없다");
  return command;
};
export const READ_ONLY_TOOLS = [
  "view_image",
  "list_mcp_resources",
  "list_mcp_resource_templates",
  "read_mcp_resource",
];
const directoryFields = [
  "workdir",
  "cwd",
  "working_directory",
  "workingDirectory",
  "working_dir",
  "workingDir",
  "work_dir",
  "workDir",
  "directory",
];
export const toolDirectory = (input, root, sessionCwd) => {
  const within = (value, relative) => {
    if (
      typeof value !== "string" ||
      value.length === 0 ||
      (!relative && !isAbsolute(value))
    )
      throw new Error("도구 작업 폴더가 올바른 경로 문자열이 아니다");
    const path = realpathSync(resolve(sessionCwd, value));
    if (!(path === root || path.startsWith(`${root}/`)))
      throw new Error("도구 작업 폴더가 세션 작업 사본 밖이다");
    return path;
  };
  const fallback = within(
    input.cwd === undefined ? sessionCwd : input.cwd,
    false,
  );
  const explicit = [input.tool_input, input.tool_input?.action].flatMap(
    (args) =>
      directoryFields
        .filter(
          (field) =>
            args !== null &&
            typeof args === "object" &&
            Object.hasOwn(args, field),
        )
        .map((field) => within(args[field], true)),
  );
  if (new Set(explicit).size > 1)
    throw new Error("도구 작업 폴더 인자가 서로 다르다");
  return explicit[0] ?? fallback;
};
export const codexPreTool = (input, project, sessionCwd = process.cwd()) => {
  try {
    const { root, actor } = actorInput(input, project, sessionCwd);
    const cwd = toolDirectory(input, root, sessionCwd);
    const scoped = { ...project, ownedWorktree: root };
    if (
      ["Bash", "exec_command", "shell_command", "shell"].includes(
        input.tool_name,
      )
    ) {
      const command = toolCommand(input);
      const verdict = decideBash(
        {
          ...input,
          ...actor,
          cwd,
          tool_input: { ...input.tool_input, command },
        },
        scoped,
      );
      const findings = shellPatches(command).flatMap((patch) =>
        patchFindings(patch, cwd, actor, scoped, root),
      );
      return preToolUseOutput(
        decide([...verdict.findings, ...findings], project.lock),
      );
    }
    if (input.tool_name === "apply_patch")
      return preToolUseOutput(
        decide(
          patchFindings(
            input.tool_input?.command,
            sessionCwd,
            actor,
            scoped,
            root,
          ),
          project.lock,
        ),
      );
    if (READ_ONLY_TOOLS.includes(input.tool_name)) return undefined;
    return refusal(`허용되지 않은 도구: ${input.tool_name}`);
  } catch (error) {
    return refusal(error.message);
  }
};
