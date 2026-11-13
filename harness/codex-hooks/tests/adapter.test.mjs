import { strict as assert } from "node:assert";
import { test } from "node:test";
import { patchPaths, codexPreTool } from "../adapter.mjs";
import {
  createFixture,
  projectFor,
} from "../../claude-hooks/tests/fixture.mjs";

test("patch targets include additions removals and both sides of moves", () => {
  assert.deepEqual(
    patchPaths(
      "*** Begin Patch\n*** Add File: a\n+x\n*** Update File: b\n*** Move to: c\n@@\n-x\n+y\n*** Delete File: d\n*** End Patch",
    ),
    ["a", "b", "c", "d"],
  );
  assert.throws(() => patchPaths("not a patch"));
});

test("Codex denies forbidden shell commands through the shared core", () => {
  const f = createFixture();
  assert.equal(
    codexPreTool(
      {
        tool_name: "Bash",
        cwd: f.worktree,
        tool_input: { command: "git commit --no-verify" },
      },
      projectFor(f),
      f.worktree,
    ).hookSpecificOutput.permissionDecision,
    "deny",
  );
});

test("Codex checks every patch target before any edit and honors the lock", () => {
  const f = createFixture();
  const input = {
    tool_name: "apply_patch",
    cwd: f.worktree,
    tool_input: {
      command:
        "*** Begin Patch\n*** Add File: crates/domain/src/new.rs\n+x\n*** Update File: justfile\n@@\n-x\n+y\n*** End Patch",
    },
  };
  assert.equal(
    codexPreTool(input, projectFor(f, { lock: "on" }), f.worktree)
      .hookSpecificOutput.permissionDecision,
    "deny",
  );
  assert.match(
    codexPreTool(input, projectFor(f), f.worktree).systemMessage,
    /잠금/,
  );
});

test("Codex rejects nested agents and malformed tool inputs", () => {
  const f = createFixture();
  for (const input of [
    { tool_name: "spawn_agent", tool_input: {} },
    { tool_name: "apply_patch", tool_input: { command: "invalid" } },
    { tool_name: "Bash", tool_input: {} },
  ]) {
    assert.equal(
      codexPreTool({ ...input, cwd: f.worktree }, projectFor(f), f.worktree)
        .hookSpecificOutput.permissionDecision,
      "deny",
    );
  }
});
