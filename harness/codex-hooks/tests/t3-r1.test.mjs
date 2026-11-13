import { strict as assert } from "node:assert";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { orchestrationFindings } from "../../../.claude/hooks/lib/orchestration.mjs";
const rules = JSON.parse(
  readFileSync(
    new URL("../../claude-hooks/agent-rules.json", import.meta.url),
    "utf8",
  ),
);
const base = "a".repeat(40);
const revised = "b".repeat(40);
const facts = {
  baseTip: base,
  resolveCommit: (value) => value,
  passedTargets: new Map(),
  reportDigests: new Set(),
  t3Threads: new Map([["owned", { branch: "topic" }]]),
  worktreeForBranch: () => "/owned/topic",
  isAncestor: () => true,
  headOf: () => revised,
};
const launch = {
  tool_name: "mcp__t3_code__t3_thread_launch",
  tool_input: {
    title: "구현",
    modelSelection: { provider: "codex", model: "gpt-6.1-sol" },
    workspaceStrategy: {
      type: "worktree",
      branch: "topic",
      baseRef: base,
      startFromOrigin: false,
    },
    message:
      "- 기준 커밋: " +
      base +
      ", 브랜치 " +
      String.fromCharCode(96) +
      "topic" +
      String.fromCharCode(96),
  },
};
test("R8 T3 launch rejects cross project environment elevated mode arbitrary keys and options", () => {
  assert.deepEqual(orchestrationFindings(launch, rules, facts), []);
  for (const extra of [
    { projectId: "elsewhere" },
    { environmentId: "elsewhere" },
    { runtimeMode: "full-access" },
    { interactionMode: "unsafe" },
    { scratch: true },
    { role: "general" },
  ])
    assert.ok(
      orchestrationFindings(
        { ...launch, tool_input: { ...launch.tool_input, ...extra } },
        rules,
        facts,
      ).length,
    );
  const invalid = structuredClone(launch);
  invalid.tool_input.modelSelection.options = { sandbox: "disabled" };
  assert.ok(orchestrationFindings(invalid, rules, facts).length);
});
test("R10 only owned threads accept followups at an existing descendant without passedTargets", () => {
  const input = {
    tool_name: "mcp__t3_code__t3_thread_send",
    tool_input: {
      threadId: "owned",
      message:
        "- 기준 커밋: " +
        revised +
        ", 브랜치 " +
        String.fromCharCode(96) +
        "topic" +
        String.fromCharCode(96),
    },
  };
  assert.deepEqual(orchestrationFindings(input, rules, facts), []);
  assert.ok(
    orchestrationFindings(
      { ...input, tool_input: { ...input.tool_input, threadId: "other" } },
      rules,
      facts,
    ).length,
  );
  for (const name of ["read", "wait"])
    assert.deepEqual(
      orchestrationFindings(
        {
          tool_name: "mcp__t3_code__t3_thread_" + name,
          tool_input: { threadId: "owned" },
        },
        rules,
        facts,
      ),
      [],
    );
});
test("T3 result parsing preserves structured and JSON text thread ownership", async () => {
  const { threadResult } =
    await import("../../../.claude/hooks/lib/orchestration.mjs");
  const value = { threadId: "owned" };
  assert.deepEqual(threadResult({ structuredContent: value }), value);
  assert.deepEqual(
    threadResult({ content: [{ type: "text", text: JSON.stringify(value) }] }),
    value,
  );
  assert.equal(
    threadResult({ content: [{ type: "text", text: "invalid" }] }),
    undefined,
  );
});

test("R8 legacy delegation rejects target project environment options and unapproved roles", () => {
  const input = {
    tool_name: "mcp__t3_code__delegate_task",
    tool_input: {
      task: launch.tool_input.message,
      target: { driverKind: "codex", model: "gpt-6.1-sol" },
      isolation: "worktree",
    },
  };
  assert.deepEqual(orchestrationFindings(input, rules, facts), []);
  for (const target of [
    { projectId: "other" },
    { environmentId: "other" },
    { options: { sandbox: "disabled" } },
    { model: "other" },
  ]) {
    const invalid = structuredClone(input);
    Object.assign(invalid.tool_input.target, target);
    assert.ok(orchestrationFindings(invalid, rules, facts).length);
  }
  for (const extra of [
    { runtimeMode: "full-access" },
    { role: "general" },
    { interactionMode: "unsafe" },
    { environmentId: "other" },
  ]) {
    const invalid = structuredClone(input);
    Object.assign(invalid.tool_input, extra);
    assert.ok(orchestrationFindings(invalid, rules, facts).length);
  }
});
