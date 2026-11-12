import { LOCK } from "./project.mjs";

export const DECISION = Object.freeze({
  allow: "allow",
  warn: "warn",
  deny: "deny",
});

const STOP =
  "하네스가 막은 것이다. 같은 일을 다른 방법으로 하려 하지 말고 멈춘 뒤 이유를 보고하라 (docs/harness.md 8장).";

const WARN =
  "잠금 스위치(harness/claude-hooks/lock.txt)가 꺼져 있어 경고만 한다. H-20에서 잠금이 켜지면 거부된다.";

export const decide = (findings, lock) => {
  const refusing = findings.filter(
    (finding) => !finding.gated || lock !== LOCK.off,
  );
  if (refusing.length > 0) {
    return { decision: DECISION.deny, findings: refusing };
  }
  return findings.length > 0
    ? { decision: DECISION.warn, findings }
    : { decision: DECISION.allow, findings: [] };
};

const describe = (findings) =>
  findings.map((finding) => `[${finding.rule}] ${finding.message}`).join("\n");

export const preToolUseOutput = (verdict) => {
  if (verdict.decision === DECISION.deny) {
    return {
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: `${describe(verdict.findings)}\n${STOP}`,
      },
    };
  }
  if (verdict.decision === DECISION.warn) {
    const text = `${describe(verdict.findings)}\n${WARN}`;
    return {
      systemMessage: text,
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        additionalContext: text,
      },
    };
  }
  return undefined;
};
