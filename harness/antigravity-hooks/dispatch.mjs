import { createHash } from "node:crypto";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { stateDirectory, writeRecord } from "../../.claude/hooks/lib/state.mjs";
import { sessionProject } from "../codex-hooks/session.mjs";
import {
  antigravityPostTool,
  antigravityPreTool,
  antigravityStop,
  refusal,
} from "./adapter.mjs";

const hooksInstalled = (common) => {
  const preCommit = join(common, "hooks", "pre-commit");
  if (!existsSync(preCommit)) return false;
  try {
    return readFileSync(preCommit, "utf8").includes("lefthook");
  } catch {
    return false;
  }
};

const threadIdKind = (value) => {
  if (typeof value !== "string" || value.trim() === "") return "absent";
  if (/^thread:delegated-task:/iu.test(value.trim())) return "delegated";
  if (/^mcp:launch:/iu.test(value.trim())) return "launch";
  return "other";
};

const safeRecord = (record, kind, id, payload) => {
  try {
    record(stateDirectory(), kind, id, payload);
  } catch {
    return;
  }
};

export const dispatchEvent = (
  input,
  session,
  record = writeRecord,
  extraFacts = {},
  cliEvent = undefined,
) => {
  if (
    cliEvent !== undefined &&
    input?.hookEventName !== undefined &&
    input.hookEventName !== cliEvent
  ) {
    const mismatch = `Antigravity 훅 이벤트 불일치: CLI=${String(cliEvent)}, stdin=${String(input.hookEventName)}`;
    return cliEvent === "Stop"
      ? { decision: "continue", reason: mismatch }
      : refusal(mismatch);
  }
  const event = cliEvent ?? input?.hookEventName;
  if (event === "PreToolUse") {
    const output = antigravityPreTool(
      input,
      session.project,
      session.root,
      extraFacts,
    );
    const id = input?.toolCall?.id;
    if (typeof id === "string" && id.length > 0) {
      safeRecord(
        record,
        "antigravityaudit",
        createHash("sha256").update(id).digest("hex"),
        {
          event: "PreToolUse",
          tool: input?.toolCall?.name,
          decision: output.decision,
          worktree: session.root,
          threadIdKind: threadIdKind(
            Object.hasOwn(extraFacts, "threadId")
              ? extraFacts.threadId
              : process.env.T3CODE_THREAD_ID,
          ),
        },
      );
    }
    return output;
  }
  if (event === "PostToolUse") {
    return antigravityPostTool(input);
  }
  if (event === "Stop") {
    return antigravityStop(input, session, {
      hooksReady: hooksInstalled(session.common),
      ...extraFacts,
    });
  }
  return refusal(`알 수 없는 Antigravity 훅 이벤트: ${String(event)}`);
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  let input;
  const cliEvent = process.argv[2];
  const fallbackEvent = cliEvent ?? "PreToolUse";
  try {
    input = JSON.parse(readFileSync(0, "utf8"));
    const session = sessionProject(process.cwd());
    const output = dispatchEvent(input, session, writeRecord, {}, cliEvent);
    process.stdout.write(`${JSON.stringify(output)}\n`);
  } catch (error) {
    const reason = `Antigravity 하네스 오류: ${error.message}`;
    process.stderr.write(`${reason}\n`);
    const event = cliEvent ?? input?.hookEventName ?? fallbackEvent;
    const output =
      event === "PostToolUse"
        ? {}
        : event === "Stop"
          ? { decision: "continue", reason }
          : refusal(reason);
    process.stdout.write(`${JSON.stringify(output)}\n`);
  }
}
