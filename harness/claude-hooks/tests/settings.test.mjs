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
  env: {
    CLAUDE_CODE_FORK_SUBAGENT: "0",
    CLAUDE_CODE_DISABLE_BACKGROUND_TASKS: "1",
    CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH: "1",
    CLAUDE_CODE_STOP_HOOK_BLOCK_CAP: "8",
  },
  permissions: {
    disableBypassPermissionsMode: "disable",
    deny: [
      "Edit(/harness/claude-hooks/lock.txt)",
      "Edit(/.claude/settings.local.json)",
      "Agent(fork)",
      "SendMessage",
      "Read(~/.claude/projects/**)",
      "Read(~/.claude-*/projects/**)",
      "Read(~/.local/state/stock-copilot-harness/**)",
      "Workflow",
      "RemoteTrigger",
      "CronCreate",
      "mcp__t3-code__create_threads",
      "mcp__t3-code__t3_thread_launch",
      "mcp__t3-code__t3_thread_send",
      "mcp__t3-code__schedule_task",
      "mcp__t3-code__run_scheduled_task_now",
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
      {
        matcher: "Agent|SendMessage|mcp__t3[-_]code__delegate_task",
        hooks: [
          hook("guard-orchestration.mjs", { onFailure: "block", timeout: 10 }),
        ],
      },
    ],
    PostToolUse: [
      {
        matcher: "Edit|Write",
        hooks: [hook("post-edit.mjs", { onFailure: "block", timeout: 300 })],
      },
      {
        matcher: "Agent",
        hooks: [hook("agent-result.mjs", { onFailure: "block", timeout: 30 })],
      },
    ],
    SubagentStart: [
      {
        matcher: "implementer|adversarial-reviewer|review-critic",
        hooks: [
          hook("subagent-start.mjs", { onFailure: "block", timeout: 180 }),
        ],
      },
    ],
    SubagentStop: [
      {
        matcher: "implementer",
        hooks: [hook("implementer-stop.mjs", { timeout: 3600 })],
      },
      {
        matcher: "adversarial-reviewer|review-critic",
        hooks: [hook("reviewer-stop.mjs", { timeout: 300 })],
      },
    ],
    ConfigChange: [
      {
        matcher: "user_settings|project_settings|local_settings|skills",
        hooks: [hook("config-change.mjs", { onFailure: "block", timeout: 10 })],
      },
    ],
  },
};

test(".claude/settings.json is exactly the harness settings", () => {
  assert.deepEqual(readJson(".claude/settings.json"), EXPECTED_SETTINGS);
});

test("the settings carry exactly the nine documented harness hooks", () => {
  const groups = Object.entries(EXPECTED_SETTINGS.hooks).flatMap(
    ([event, list]) => list.map((group) => `${event}(${group.matcher})`),
  );
  assert.deepEqual(groups, [
    "PreToolUse(Edit|Write|NotebookEdit)",
    "PreToolUse(Bash)",
    "PreToolUse(Agent|SendMessage|mcp__t3[-_]code__delegate_task)",
    "PostToolUse(Edit|Write)",
    "PostToolUse(Agent)",
    "SubagentStart(implementer|adversarial-reviewer|review-critic)",
    "SubagentStop(implementer)",
    "SubagentStop(adversarial-reviewer|review-critic)",
    "ConfigChange(user_settings|project_settings|local_settings|skills)",
  ]);
  assert.equal(readJson(".claude/settings.json").sandbox, undefined);
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
