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
    role: "implementation",
    target: {
      driverKind: "antigravity",
      providerInstanceId: "antigravity",
      model: "gemini-4-neon",
      options: { thought_level: "xhigh" },
    },
    task: `- 기준 커밋: ${base}, 브랜치 \`h-41\``,
  },
};
const criticCall = {
  tool_name: "mcp__t3_code__delegate_task",
  tool_input: {
    isolation: "worktree",
    role: "review",
    target: {
      driverKind: "antigravity",
      providerInstanceId: "antigravity",
      model: "gemini-4-neon",
      options: { thought_level: "high" },
    },
    task: `- 기준 커밋: ${base}, 브랜치 \`h-41\`\n- 대상 SHA: ${"b".repeat(40)}`,
  },
};

test("only T3 worktree delegation is allowed and native reviews are denied", () => {
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
  const input = structuredClone(criticCall);
  input.tool_input.target = {
    driverKind: "claudeAgent",
    model: "claude-opus-5-5",
    providerInstanceId: "claudeAgent_claude2",
  };
  assert.deepEqual(orchestrationFindings(input, rules, facts), []);
  for (const target of [
    { model: "claude-sonnet-5-5" },
    { providerInstanceId: "claudeAgent" },
    { providerInstanceId: undefined },
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
  const input = structuredClone(criticCall);
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

test("review delegation rejects implementation reports missing targets and wrong target heads", () => {
  const input = structuredClone(criticCall);
  assert.deepEqual(orchestrationFindings(input, rules, facts), []);
  for (const task of [
    call.tool_input.task,
    input.tool_input.task.replace("b".repeat(40), "d".repeat(40)),
    input.tool_input.task + "\n완료\n완료 기준별 충족 근거",
    input.tool_input.task + "\n하네스 기록: 검증",
  ])
    assert.equal(
      orchestrationFindings(
        { ...input, tool_input: { ...input.tool_input, task } },
        rules,
        facts,
      ).length,
      1,
    );
});
