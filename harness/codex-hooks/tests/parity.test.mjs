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
    const project = {
      ...projectFor(fixture, { lock: item.lock ?? "off" }),
      ownedWorktree: cwd,
    };
    assert.deepEqual(
      codexPreTool(input, project, cwd),
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
    const project = {
      ...projectFor(fixture, { lock: item.lock ?? "off" }),
      ownedWorktree: cwd,
    };
    const input = {
      tool_name: "apply_patch",
      cwd,
      tool_input: {
        command: `*** Begin Patch\n*** Add File: ${path}\n+x\n*** End Patch`,
      },
    };
    if (!path.startsWith("/") || /[\s\p{Cc}]/u.test(path)) continue;
    if (!path.startsWith(cwd + "/")) {
      assert.equal(
        codexPreTool(input, project, cwd).hookSpecificOutput.permissionDecision,
        "deny",
      );
      continue;
    }
    assert.deepEqual(
      codexPreTool(input, project, cwd),
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
test("R16 moving deleting relative indented and trailing-space patches keep protected targets denied", () => {
  const root = fixture.worktree;
  const snippets = [
    "*** Update File: justfile\n*** Move to: crates/domain/src/moved.rs",
    "*** Update File: crates/domain/src/lib.rs\n*** Move to: justfile",
    "*** Delete File: justfile",
    " *** Update File: justfile",
    "*** Update File: justfile ",
    "*** Update File: ../repo/justfile",
  ];
  for (const header of snippets) {
    const input = {
      tool_name: "apply_patch",
      cwd: root,
      tool_input: {
        command: "*** Begin Patch\n" + header + "\n@@\n-x\n+y\n*** End Patch",
      },
    };
    assert.equal(
      codexPreTool(input, projectFor(fixture, { lock: "on" }), root)
        .hookSpecificOutput.permissionDecision,
      "deny",
      header,
    );
  }
});
