import { strict as assert } from "node:assert";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { lintFeedback } from "../../../.claude/hooks/lib/post-edit.mjs";
import { worktreesOf } from "../../../.claude/hooks/lib/project.mjs";
import { readJson, repoRoot, runHook } from "./fixture.mjs";

const hook = (script, extra = {}) => ({
  type: "command",
  command: `node "$CLAUDE_PROJECT_DIR/.claude/hooks/${script}"`,
  ...extra,
});

const EXPECTED_SETTINGS = {
  permissions: {
    deny: [
      "Edit(/harness/claude-hooks/lock.txt)",
      "Edit(/.claude/settings.local.json)",
    ],
  },
  hooks: {
    PreToolUse: [
      {
        matcher: "Edit|Write|NotebookEdit",
        hooks: [hook("guard-edit.mjs", { onFailure: "block", timeout: 10 })],
      },
      {
        matcher: "Bash",
        hooks: [hook("guard-bash.mjs", { onFailure: "block", timeout: 10 })],
      },
    ],
    PostToolUse: [
      {
        matcher: "Edit|Write",
        hooks: [hook("post-edit.mjs", { timeout: 300 })],
      },
    ],
    ConfigChange: [
      {
        matcher: "project_settings|local_settings|skills",
        hooks: [hook("config-change.mjs", { onFailure: "block", timeout: 10 })],
      },
    ],
  },
};

test(".claude/settings.json is exactly the harness settings", () => {
  assert.deepEqual(readJson(".claude/settings.json"), EXPECTED_SETTINGS);
});

test("every hook command runs a script that exists in .claude/hooks", () => {
  Object.values(EXPECTED_SETTINGS.hooks)
    .flat()
    .flatMap((group) => group.hooks)
    .forEach((entry) => {
      const script = /\/\.claude\/hooks\/([a-z-]+\.mjs)"$/u.exec(entry.command);
      assert.ok(script !== null, entry.command);
      assert.ok(existsSync(join(repoRoot, ".claude/hooks", script[1])));
    });
});

test("the lock switch is off until H-20 and denied to every edit tool", () => {
  assert.equal(
    readFileSync(join(repoRoot, "harness/claude-hooks/lock.txt"), "utf8"),
    "off\n",
  );
  assert.ok(
    EXPECTED_SETTINGS.permissions.deny.includes(
      "Edit(/harness/claude-hooks/lock.txt)",
    ),
  );
});

test("post-edit returns decision block with the lint-file output when it fails", () => {
  const project = { worktrees: [repoRoot] };
  const path = join(repoRoot, "crates/domain/src/lib.rs");
  const calls = [];
  const failing = lintFeedback(
    { tool_input: { file_path: path } },
    project,
    (root, given) => {
      calls.push([root, given]);
      return { status: 1, output: "lint-file: 위반 (rustfmt)" };
    },
  );
  assert.deepEqual(calls, [[repoRoot, path]]);
  assert.equal(failing.decision, "block");
  assert.match(
    failing.reason,
    /crates\/domain\/src\/lib\.rs[\s\S]*위반 \(rustfmt\)/u,
  );
  const passing = lintFeedback(
    { tool_input: { file_path: path } },
    project,
    () => ({
      status: 0,
      output: "",
    }),
  );
  assert.equal(passing, undefined);
  const outside = lintFeedback(
    { tool_input: { file_path: "/etc/hosts" } },
    project,
    () => {
      throw new Error("must not lint outside the worktree");
    },
  );
  assert.equal(outside, undefined);
});

test("post-edit.mjs runs the real just lint-file and blocks on its failure", () => {
  assert.ok(worktreesOf(repoRoot).includes(repoRoot));
  const clean = runHook("post-edit.mjs", {
    hook_event_name: "PostToolUse",
    tool_name: "Write",
    tool_input: { file_path: join(repoRoot, ".claude/hooks/guard-edit.mjs") },
  });
  assert.equal(clean.status, 0, clean.stderr);
  assert.equal(clean.stdout, "");
  const missing = runHook("post-edit.mjs", {
    hook_event_name: "PostToolUse",
    tool_name: "Write",
    tool_input: {
      file_path: join(repoRoot, "crates/domain/src/missing-probe.rs"),
    },
  });
  assert.equal(missing.status, 0, missing.stderr);
  assert.equal(missing.output.decision, "block");
  assert.match(missing.output.reason, /없는 파일이다/u);
});

test("config-change.mjs blocks a settings change made during the session", () => {
  const blocked = runHook("config-change.mjs", {
    hook_event_name: "ConfigChange",
    source: "project_settings",
    file_path: join(repoRoot, ".claude/settings.json"),
  });
  assert.equal(blocked.status, 0, blocked.stderr);
  assert.equal(blocked.output.decision, "block");
  assert.match(blocked.output.reason, /project_settings[\s\S]*다시 시작/u);
});
