import { strict as assert } from "node:assert";
import { test } from "node:test";
import { codexPreTool } from "../adapter.mjs";
import { decideBash } from "../../../.claude/hooks/lib/bash-guard.mjs";
import { decideEdit } from "../../../.claude/hooks/lib/edit-guard.mjs";
import { preToolUseOutput } from "../../../.claude/hooks/lib/verdict.mjs";
import {
  createFixture,
  projectFor,
  placeholders,
  readJson,
  roleInput,
} from "../../claude-hooks/tests/fixture.mjs";

const fixture = createFixture();
const fill = placeholders(fixture);

test("Claude and Codex adapters agree on the shared shell corpus for both operating roles", () => {
  for (const item of readJson("harness/claude-hooks/bash-cases.json").filter(
    (item) =>
      item.role === undefined || ["main", "implementer"].includes(item.role),
  )) {
    const cwd = item.role === "implementer" ? fixture.worktree : fixture.root;
    const input = {
      tool_name: "Bash",
      cwd,
      tool_input: { command: fill(item.command) },
    };
    const project = projectFor(fixture, { lock: item.lock ?? "off" });
    assert.deepEqual(
      codexPreTool(input, project),
      preToolUseOutput(
        decideBash({ ...input, ...roleInput(item.role) }, project),
      ),
      item.command,
    );
  }
});

test("Claude and Codex adapters agree on every representable edit example", () => {
  for (const item of readJson("harness/claude-hooks/edit-cases.json").filter(
    (item) =>
      typeof item.path === "string" &&
      (item.role === undefined || ["main", "implementer"].includes(item.role)),
  )) {
    const path = fill(item.path);
    const cwd = item.role === "implementer" ? fixture.worktree : fixture.root;
    const project = projectFor(fixture, { lock: item.lock ?? "off" });
    const input = {
      tool_name: "apply_patch",
      cwd,
      tool_input: {
        command: `*** Begin Patch\n*** Add File: ${path}\n+x\n*** End Patch`,
      },
    };
    if (!path.startsWith("/")) continue;
    assert.deepEqual(
      codexPreTool(input, project),
      preToolUseOutput(
        decideEdit(
          { ...roleInput(item.role), tool_input: { file_path: path } },
          project,
        ),
      ),
      path,
    );
  }
});
