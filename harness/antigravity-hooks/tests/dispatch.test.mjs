import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { chmodSync, cpSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { after, test } from "node:test";
import { dispatchEvent } from "../dispatch.mjs";
import { sessionProject } from "../../codex-hooks/session.mjs";
import { createFixture, repoRoot } from "../../claude-hooks/tests/fixture.mjs";

const f = createFixture();
for (const path of [
  "harness/claude-hooks",
  "harness/agent-writable.txt",
  "harness/sandbox/policy.json",
  "harness/arch-check/rules.toml",
]) {
  mkdirSync(join(f.root, path, ".."), { recursive: true });
  cpSync(join(repoRoot, path), join(f.root, path), { recursive: true });
}
const wtAgents = join(f.worktree, ".agents");
mkdirSync(wtAgents, { recursive: true });

after(() => rmSync(f.base, { recursive: true, force: true }));

const runCli = (stdinText, cwd = wtAgents, eventArg = "PreToolUse") =>
  spawnSync(
    process.execPath,
    [join(repoRoot, "harness/antigravity-hooks/dispatch.mjs"), eventArg],
    {
      cwd,
      input: stdinText,
      encoding: "utf8",
      env: {
        ...process.env,
        STOCK_COPILOT_HARNESS_STATE: join(f.base, "state"),
      },
    },
  );

test("dispatchEvent handles PreToolUse PostToolUse Stop and records audit", () => {
  const session = sessionProject(wtAgents);
  const audits = [];
  const record = (...args) => audits.push(args.at(-1));

  const allowed = dispatchEvent(
    {
      hookEventName: "PreToolUse",
      cwd: f.worktree,
      modelName: "gemini-4-neon-xhigh",
      toolCall: {
        id: "audit-1",
        name: "read_file",
        args: { Path: join(f.worktree, "docs/notes.md") },
      },
    },
    session,
    record,
    { branch: "h-61" },
  );
  assert.equal(allowed.decision, "allow");
  assert.equal(audits.length, 1);
  assert.equal(audits[0].decision, "allow");

  const denied = dispatchEvent(
    {
      hookEventName: "PreToolUse",
      cwd: f.worktree,
      modelName: "gemini-4-neon-high",
      toolCall: {
        id: "audit-2",
        name: "edit_file",
        args: { Path: join(f.worktree, "docs/notes.md") },
      },
    },
    session,
    record,
    { branch: "h-61" },
  );
  assert.equal(denied.decision, "deny");
  assert.equal(audits[1].decision, "deny");

  assert.deepEqual(
    dispatchEvent(
      {
        hookEventName: "PostToolUse",
        toolCall: { id: "audit-1", name: "read_file", args: {} },
      },
      session,
      record,
    ),
    {},
  );

  const dirtyReviewerStop = dispatchEvent(
    {
      hookEventName: "Stop",
      cwd: f.worktree,
      modelName: "gemini-4-neon-high",
      terminationReason: "NO_TOOL_CALL",
      fullyIdle: true,
      continuationActive: false,
    },
    session,
    record,
    { branch: "h-61", trackedDirty: " M docs/notes.md" },
  );
  assert.equal(dirtyReviewerStop.decision, "continue");

  const cleanReviewerStop = dispatchEvent(
    {
      hookEventName: "Stop",
      cwd: f.worktree,
      modelName: "gemini-4-neon-high",
      terminationReason: "NO_TOOL_CALL",
      fullyIdle: true,
      continuationActive: false,
    },
    session,
    record,
    { branch: "h-61", trackedDirty: "" },
  );
  assert.equal(cleanReviewerStop.decision, "allow");

  const dirtyImplementerStop = dispatchEvent(
    {
      hookEventName: "Stop",
      cwd: f.worktree,
      modelName: "gemini-4-neon-xhigh",
      terminationReason: "NO_TOOL_CALL",
      fullyIdle: true,
      continuationActive: false,
    },
    session,
    record,
    { branch: "h-61", hooksReady: true, dirty: " M crates/domain/src/lib.rs" },
  );
  assert.equal(dirtyImplementerStop.decision, "continue");

  const continuedStop = dispatchEvent(
    {
      hookEventName: "Stop",
      cwd: f.worktree,
      modelName: "gemini-4-neon-xhigh",
      terminationReason: "NO_TOOL_CALL",
      fullyIdle: true,
      continuationActive: true,
    },
    session,
    record,
    { branch: "h-61", hooksReady: true, dirty: " M crates/domain/src/lib.rs" },
  );
  assert.equal(continuedStop.decision, "allow");

  const missingHooksStop = dispatchEvent(
    {
      hookEventName: "Stop",
      cwd: f.worktree,
      modelName: "gemini-4-neon-xhigh",
      terminationReason: "NO_TOOL_CALL",
      fullyIdle: true,
      continuationActive: false,
    },
    session,
    record,
    { branch: "h-61", hooksReady: false, dirty: "" },
  );
  assert.equal(missingHooksStop.decision, "continue");

  const errorTerminationStop = dispatchEvent(
    {
      hookEventName: "Stop",
      cwd: f.worktree,
      modelName: "gemini-4-neon-xhigh",
      terminationReason: "ERROR",
      fullyIdle: true,
      continuationActive: false,
    },
    session,
    record,
    { branch: "h-61", hooksReady: false, dirty: " M x" },
  );
  assert.equal(errorTerminationStop.decision, "allow");

  assert.equal(
    dispatchEvent({ hookEventName: "UnknownEvent" }, session, record).decision,
    "deny",
  );
});

test("dispatch CLI fails closed on malformed JSON and enforces PreToolUse from .agents cwd", () => {
  const brokenPre = runCli("{not json", wtAgents, "PreToolUse");
  assert.equal(brokenPre.status, 0);
  assert.equal(JSON.parse(brokenPre.stdout).decision, "deny");

  const brokenStop = runCli("{not json", wtAgents, "Stop");
  assert.equal(brokenStop.status, 0);
  assert.equal(JSON.parse(brokenStop.stdout).decision, "continue");

  const deniedCli = runCli(
    JSON.stringify({
      hookEventName: "PreToolUse",
      cwd: f.worktree,
      modelName: "gemini-4-neon-xhigh",
      toolCall: {
        id: "cli-1",
        name: "write_file",
        args: {
          Path: join(f.worktree, ".claude/settings.local.json"),
          Content: "{}",
        },
      },
    }),
    wtAgents,
    "PreToolUse",
  );
  assert.equal(deniedCli.status, 0, deniedCli.stderr);
  assert.equal(JSON.parse(deniedCli.stdout).decision, "deny");

  writeFileSync(
    join(f.root, ".git/hooks/pre-commit"),
    "#!/bin/sh\nlefthook run pre-commit\n",
  );
  const cleanStopCli = runCli(
    JSON.stringify({
      hookEventName: "Stop",
      cwd: f.worktree,
      modelName: "gemini-4-neon-xhigh",
      terminationReason: "NO_TOOL_CALL",
      fullyIdle: true,
      continuationActive: false,
    }),
    wtAgents,
    "Stop",
  );
  assert.equal(cleanStopCli.status, 0, cleanStopCli.stderr);
  assert.equal(JSON.parse(cleanStopCli.stdout).decision, "allow");

  const spoofedEvent = runCli(
    JSON.stringify({
      hookEventName: "PostToolUse",
      cwd: f.worktree,
      modelName: "gemini-4-neon-xhigh",
      toolCall: {
        id: "cli-spoof",
        name: "write_file",
        args: {
          Path: join(f.worktree, ".agents/hooks.json"),
          Content: "{}",
        },
      },
    }),
    wtAgents,
    "PreToolUse",
  );
  assert.equal(spoofedEvent.status, 0, spoofedEvent.stderr);
  assert.equal(JSON.parse(spoofedEvent.stdout).decision, "deny");

  const benignSpoofedEvent = runCli(
    JSON.stringify({
      hookEventName: "PostToolUse",
      cwd: f.worktree,
      modelName: "gemini-4-neon-xhigh",
      toolCall: {
        id: "cli-spoof-benign",
        name: "read_file",
        args: {
          Path: join(f.worktree, "docs/notes.md"),
        },
      },
    }),
    wtAgents,
    "PreToolUse",
  );
  assert.equal(benignSpoofedEvent.status, 0, benignSpoofedEvent.stderr);
  const benignSpoofedParsed = JSON.parse(benignSpoofedEvent.stdout);
  assert.equal(benignSpoofedParsed.decision, "deny");
  assert.match(benignSpoofedParsed.reason, /이벤트 불일치/u);
});

test("Stop fails closed with continue when real git status fails without injected facts", () => {
  const session = sessionProject(wtAgents);
  const worktreeGitDir = join(f.root, ".git/worktrees/worktree");
  const indexPath = join(worktreeGitDir, "index");
  chmodSync(indexPath, 0o000);
  try {
    const implStop = dispatchEvent(
      {
        hookEventName: "Stop",
        cwd: f.worktree,
        modelName: "gemini-4-neon-xhigh",
        terminationReason: "NO_TOOL_CALL",
        fullyIdle: true,
        continuationActive: false,
      },
      session,
      () => {},
      { branch: "h-61", hooksReady: true },
      "Stop",
    );
    assert.equal(implStop.decision, "continue");
    assert.match(implStop.reason, /git status/u);

    const revStop = dispatchEvent(
      {
        hookEventName: "Stop",
        cwd: f.worktree,
        modelName: "gemini-4-neon-high",
        terminationReason: "NO_TOOL_CALL",
        fullyIdle: true,
        continuationActive: false,
      },
      session,
      () => {},
      { branch: "h-61" },
      "Stop",
    );
    assert.equal(revStop.decision, "continue");
    assert.match(revStop.reason, /git status/u);
  } finally {
    chmodSync(indexPath, 0o644);
  }
});
