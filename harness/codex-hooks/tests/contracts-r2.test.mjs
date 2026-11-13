import { strict as assert } from "node:assert";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync, rmSync, readFileSync } from "node:fs";
import { join } from "node:path";
import * as config from "../config.mjs";
import { currentHashes, hashProblems } from "../../claude-hooks/hashes.mjs";
import { orchestrationFindings } from "../../../.claude/hooks/lib/orchestration.mjs";
import { createFixture, repoRoot } from "../../claude-hooks/tests/fixture.mjs";
const rules = JSON.parse(
  readFileSync(join(repoRoot, "harness/claude-hooks/agent-rules.json"), "utf8"),
);
const base = "a".repeat(40);
const remote = "b".repeat(40);
const brief = (value) => "- 기준 커밋: " + value + ", 브랜치 `topic`";
const facts = {
  baseTip: base,
  originTip: remote,
  resolveCommit: (value) => (value === "origin/master" ? remote : value),
  passedTargets: new Map(),
  worktreeForBranch: () => "/topic",
  t3Threads: new Map([["owned", { branch: "topic" }]]),
  isAncestor: () => true,
  headOf: () => base,
};
test("R2 wrapper emits event appropriate closed failure for broken HEAD and module import", () => {
  const f = createFixture();
  try {
    mkdirSync(join(f.root, "harness/codex-hooks"), { recursive: true });
    writeFileSync(
      join(f.root, "harness/codex-hooks/dispatch.mjs"),
      'import "./missing.mjs";\n',
    );
    const admin = spawnSync("git", ["rev-parse", "--absolute-git-dir"], {
      cwd: f.worktree,
      encoding: "utf8",
    }).stdout.trim();
    for (const broken of ["import", "HEAD"]) {
      if (broken === "HEAD") writeFileSync(join(admin, "HEAD"), "broken\n");
      for (const event of ["PreToolUse", "Stop"]) {
        const command = config.eventHookCommand?.(event) ?? config.hookCommand;
        const run = spawnSync("/bin/sh", ["-c", command], {
          cwd: f.worktree,
          input: JSON.stringify({ hook_event_name: event }),
          encoding: "utf8",
        });
        assert.equal(run.status, 0, broken + " " + event + ": " + run.stderr);
        const output = JSON.parse(run.stdout);
        assert.equal(
          event === "Stop"
            ? output.decision
            : output.hookSpecificOutput.permissionDecision,
          event === "Stop" ? "block" : "deny",
        );
        assert.ok(run.stderr.length);
      }
    }
    const brokenStop = spawnSync(
      process.execPath,
      [join(repoRoot, "harness/codex-hooks/dispatch.mjs"), "Stop"],
      {
        cwd: f.worktree,
        input: JSON.stringify({ hook_event_name: "Stop" }),
        encoding: "utf8",
      },
    );
    assert.equal(brokenStop.status, 0);
    assert.equal(JSON.parse(brokenStop.stdout).decision, "block");
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
test("R2 all T3 mutations are gated and queued sends are refused", () => {
  const settings = JSON.parse(
    readFileSync(join(repoRoot, ".claude/settings.json"), "utf8"),
  );
  const matcher = settings.hooks.PreToolUse.find((item) =>
    item.hooks[0].command.includes("guard-orchestration"),
  ).matcher;
  for (const tool of [
    "t3_thread_configure",
    "t3_queue",
    "unknown_future_tool",
  ]) {
    const name = "mcp__t3-code__" + tool;
    assert.ok(new RegExp(matcher, "u").test(name), name);
    assert.ok(
      orchestrationFindings({ tool_name: name, tool_input: {} }, rules, facts)
        .length,
      name,
    );
  }
  const input = {
    tool_name: "mcp__t3_code__t3_thread_send",
    tool_input: { threadId: "owned", mode: "queue", message: brief(base) },
  };
  assert.ok(orchestrationFindings(input, rules, facts).length);
  for (const tool of [
    "t3_project_list",
    "orchestrator_capabilities",
    "t3_background_wait",
  ])
    assert.deepEqual(
      orchestrationFindings(
        { tool_name: "mcp__t3-code__" + tool, tool_input: {} },
        rules,
        facts,
      ),
      [],
    );
});
test("R2 local unpublished and origin launch bases have distinct enforced modes", () => {
  const launch = (baseRef, startFromOrigin, value) => ({
    tool_name: "mcp__t3_code__t3_thread_launch",
    tool_input: {
      message: brief(value),
      modelSelection: { provider: "codex", model: "gpt-6.1-sol" },
      workspaceStrategy: {
        type: "worktree",
        branch: "topic",
        baseRef,
        startFromOrigin,
      },
    },
  });
  assert.deepEqual(
    orchestrationFindings(launch(base, false, base), rules, facts),
    [],
  );
  assert.deepEqual(
    orchestrationFindings(launch("origin/master", true, remote), rules, facts),
    [],
  );
  for (const input of [
    launch(base, true, base),
    launch(base, undefined, base),
    launch("origin/master", true, base),
    launch("origin/master", false, remote),
    launch("origin/master", undefined, remote),
    launch("origin/other", true, remote),
  ])
    assert.ok(orchestrationFindings(input, rules, facts).length);
});
test("R2 hashes ignore untracked noise but include staged guard files", () => {
  const f = createFixture();
  try {
    const before = currentHashes(f.root);
    mkdirSync(join(f.root, ".claude/hooks"), { recursive: true });
    writeFileSync(join(f.root, ".claude/hooks/.DS_Store"), "noise");
    mkdirSync(join(f.root, "harness/codex-hooks"), { recursive: true });
    const added = "harness/codex-hooks/new.mjs";
    writeFileSync(join(f.root, added), "export const value = 1;\n");
    assert.deepEqual(hashProblems(f.root, before), []);
    assert.equal(spawnSync("git", ["add", added], { cwd: f.root }).status, 0);
    assert.deepEqual(hashProblems(f.root, before), [
      added + ": 기대값에 없는 파일이다",
    ]);
    rmSync(join(f.root, added));
    assert.ok(
      hashProblems(f.root, { ...before, [added]: "expected" }).some((value) =>
        value.startsWith(added),
      ),
    );
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
