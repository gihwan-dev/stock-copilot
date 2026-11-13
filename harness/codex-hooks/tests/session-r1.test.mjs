import { strict as assert } from "node:assert";
import { test } from "node:test";
import { cpSync, mkdirSync, writeFileSync, rmSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { join } from "node:path";
import { sessionProject } from "../session.mjs";
import { hookCommand } from "../config.mjs";
import { createFixture, repoRoot } from "../../claude-hooks/tests/fixture.mjs";
test("R4 git environment cannot redirect session identity and R6 worktree hook tampering is ignored", () => {
  const f = createFixture();
  try {
    for (const path of [
      ".claude/hooks",
      "harness/claude-hooks",
      "harness/codex-hooks",
      "harness/agent-writable.txt",
    ])
      cpSync(join(repoRoot, path), join(f.root, path), { recursive: true });
    mkdirSync(join(f.worktree, "harness/codex-hooks"), { recursive: true });
    writeFileSync(
      join(f.worktree, "harness/codex-hooks/dispatch.mjs"),
      'throw new Error("tampered");\n',
    );
    const keys = [
      "GIT_DIR",
      "GIT_WORK_TREE",
      "GIT_COMMON_DIR",
      "GIT_INDEX_FILE",
      "GIT_CONFIG_PARAMETERS",
      "GIT_CONFIG_COUNT",
    ];
    const previous = Object.fromEntries(
      keys.map((key) => [key, process.env[key]]),
    );
    try {
      keys.forEach((key) => (process.env[key] = f.outside));
      const value = sessionProject(f.worktree);
      assert.equal(value.root, f.worktree);
      assert.equal(value.project.root, f.root);
      const run = spawnSync("/bin/sh", ["-c", hookCommand], {
        cwd: f.worktree,
        input: JSON.stringify({
          hook_event_name: "PreToolUse",
          tool_name: "Bash",
          cwd: f.worktree,
          tool_input: { command: "pwd" },
        }),
        encoding: "utf8",
      });
      assert.equal(run.status, 0, run.stderr);
      assert.equal(run.stdout, "", run.stderr);
    } finally {
      for (const key of keys)
        if (previous[key] === undefined) delete process.env[key];
        else process.env[key] = previous[key];
    }
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
