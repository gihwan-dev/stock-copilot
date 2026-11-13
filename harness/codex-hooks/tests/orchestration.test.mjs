import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { orchestrationFindings } from "../../../.claude/hooks/lib/orchestration.mjs";

const rules = JSON.parse(
  readFileSync(
    new URL("../../claude-hooks/agent-rules.json", import.meta.url),
    "utf8",
  ),
);
const base = "a".repeat(40);
const facts = {
  baseTip: base,
  resolveCommit: (value) => value,
  passedTargets: new Map(),
  reportDigests: new Set(),
};
const call = {
  tool_name: "mcp__t3_code__delegate_task",
  tool_input: {
    isolation: "worktree",
    target: { driverKind: "codex", model: "gpt-6.1-sol" },
    task: `- 기준 커밋: ${base}, 브랜치 \`h-41\``,
  },
};

test("only fresh Codex worktree delegation and Claude adversarial review are allowed", () => {
  assert.deepEqual(orchestrationFindings(call, rules, facts), []);
  for (const key of ["driverKind", "model"]) {
    const input = structuredClone(call);
    delete input.tool_input.target[key];
    assert.equal(orchestrationFindings(input, rules, facts).length, 1);
  }
  for (const input of [
    { ...call, agent_id: "nested" },
    { ...call, tool_input: { ...call.tool_input, isolation: "shared" } },
    { tool_name: "Agent", tool_input: { subagent_type: "implementer" } },
    { tool_name: "Agent", tool_input: { subagent_type: "review-critic" } },
    { tool_name: "SendMessage" },
  ])
    assert.equal(orchestrationFindings(input, rules, facts).length, 1);
  assert.deepEqual(
    orchestrationFindings(
      {
        tool_name: "Agent",
        tool_input: {
          subagent_type: "adversarial-reviewer",
          description: "리뷰",
          prompt: "브리프와 대상 SHA",
        },
      },
      rules,
      facts,
    ),
    [],
  );
});
