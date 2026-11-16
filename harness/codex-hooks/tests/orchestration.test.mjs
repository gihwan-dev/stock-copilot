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
  worktreeForBranch: (branch) => (branch === "h-41" ? "/target" : undefined),
  headOf: () => "b".repeat(40),
  isAncestor: () => true,
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
  assert.equal(
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
    ).length,
    1,
  );
});

test("T3 Opus review requires the exact model role provider and worktree", () => {
  const input = structuredClone(call);
  input.tool_input.role = "review";
  input.tool_input.target = {
    driverKind: "claudeAgent",
    model: "claude-opus-5-5",
    providerInstanceId: "claudeAgent",
  };
  assert.deepEqual(orchestrationFindings(input, rules, facts), []);
  for (const target of [
    { model: "claude-sonnet-5-5" },
    { providerInstanceId: "other" },
    { options: { effort: "max" } },
  ]) {
    assert.equal(
      orchestrationFindings(
        {
          ...input,
          tool_input: {
            ...input.tool_input,
            target: { ...input.tool_input.target, ...target },
          },
        },
        rules,
        facts,
      ).length,
      1,
    );
  }
  for (const change of [
    { role: "implementation" },
    { isolation: "shared" },
    { runtimeMode: "full-access" },
  ]) {
    assert.equal(
      orchestrationFindings(
        { ...input, tool_input: { ...input.tool_input, ...change } },
        rules,
        facts,
      ).length,
      1,
    );
  }
});

test("pinned review bases survive parallel master movement but must belong to the target", () => {
  const input = structuredClone(call);
  input.tool_input.role = "review";
  const moved = { ...facts, baseTip: "c".repeat(40) };
  assert.deepEqual(orchestrationFindings(input, rules, moved), []);
  assert.equal(
    orchestrationFindings(input, rules, { ...moved, isAncestor: () => false })
      .length,
    1,
  );
  assert.equal(
    orchestrationFindings(input, rules, {
      ...moved,
      worktreeForBranch: () => undefined,
    }).length,
    1,
  );
  assert.equal(
    orchestrationFindings(input, rules, { ...moved, headOf: () => undefined })
      .length,
    1,
  );
});
