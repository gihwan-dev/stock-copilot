import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { baseVerdict } from "./agent-guard.mjs";
import { digestOf, parseBrief } from "./brief.mjs";
import { ROLE, roleOf } from "./roles.mjs";
import { STATUS, isSafeId } from "./state.mjs";

export const subagentTranscript = (input) =>
  typeof input.transcript_path === "string" && isSafeId(input.session_id)
    ? join(
        dirname(input.transcript_path),
        input.session_id,
        "subagents",
        `agent-${input.agent_id}.jsonl`,
      )
    : undefined;

const textOf = (content) =>
  typeof content === "string"
    ? content
    : Array.isArray(content)
      ? content
          .filter((part) => part !== null && typeof part === "object")
          .map((part) => (typeof part.text === "string" ? part.text : ""))
          .join("")
      : undefined;

export const subagentToolUse = (path) => {
  if (path === undefined) {
    return undefined;
  }
  try {
    const meta = JSON.parse(
      readFileSync(path.replace(/\.jsonl$/u, ".meta.json"), "utf8"),
    );
    return typeof meta.toolUseId === "string" ? meta.toolUseId : undefined;
  } catch {
    return undefined;
  }
};

export const firstPrompt = (path) => {
  if (path === undefined) {
    return undefined;
  }
  try {
    const line = readFileSync(path, "utf8").split("\n")[0];
    const entry = JSON.parse(line);
    return entry.type === "user" ? textOf(entry.message?.content) : undefined;
  } catch {
    return undefined;
  }
};

export const implementerStart = (input, prompt, facts, rules) => {
  const common = {
    kind: "implementer",
    agentId: input.agent_id,
    sessionId: input.session_id,
    startedAt: facts.now(),
    blocks: 0,
  };
  if (prompt === undefined) {
    return {
      ...common,
      status: STATUS.unchecked,
      reason:
        "브리프를 읽지 못했다 (PreToolUse 대기열과 서브에이전트 기록 모두 없음)",
    };
  }
  const brief = parseBrief(prompt);
  const verdict = baseVerdict(brief, facts, rules);
  if (verdict.problem !== undefined) {
    return {
      ...common,
      status: STATUS.unchecked,
      promptDigest: digestOf(prompt),
      reason: verdict.problem,
    };
  }
  return {
    ...common,
    status: STATUS.unchecked,
    base: verdict.commit,
    baseSource: verdict.source,
    baseTipAtStart: facts.baseTip,
    branch: brief.branch,
    worktree: facts.worktreeForBranch(brief.branch),
    promptDigest: digestOf(prompt),
  };
};

export const startKind = (input) => {
  const role = roleOf(input);
  return role === ROLE.implementer || role === ROLE.reviewer ? role : undefined;
};

export const choosePending = (entries, digest, toolUseId) => {
  const ordered = [...entries].sort((left, right) =>
    String(left.at).localeCompare(String(right.at)),
  );
  if (digest !== undefined) {
    return ordered.find((entry) => entry.digest === digest);
  }
  if (toolUseId !== undefined) {
    return ordered.find((entry) => entry.toolUseId === toolUseId);
  }
  return ordered[0];
};
