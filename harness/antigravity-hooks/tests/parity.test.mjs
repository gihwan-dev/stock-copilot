import { strict as assert } from "node:assert";
import { cpSync, mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { after, test } from "node:test";
import { antigravityPreTool } from "../adapter.mjs";
import { decideBash } from "../../../.claude/hooks/lib/bash-guard.mjs";
import { decideEdit } from "../../../.claude/hooks/lib/edit-guard.mjs";
import { DECISION } from "../../../.claude/hooks/lib/verdict.mjs";
import {
  createFixture,
  placeholders,
  projectFor,
  readJson,
  repoRoot,
  roleInput,
} from "../../claude-hooks/tests/fixture.mjs";

const fixture = createFixture();
mkdirSync(join(fixture.root, "harness/sandbox"), { recursive: true });
mkdirSync(join(fixture.root, "harness/arch-check"), { recursive: true });
cpSync(
  join(repoRoot, "harness/sandbox/policy.json"),
  join(fixture.root, "harness/sandbox/policy.json"),
);
cpSync(
  join(repoRoot, "harness/arch-check/rules.toml"),
  join(fixture.root, "harness/arch-check/rules.toml"),
);
const fill = placeholders(fixture);
const fakeHome = join(fixture.base, "parity-home");

after(() => rmSync(fixture.base, { recursive: true, force: true }));

const PRELOCK_REVIEW_RECORD = /\/docs\/reviews\/[A-Z]+-\d+\.md$/u;
const PRELOCK_GENERATOR_CMD =
  /^node\s+(?:\.\/)?harness\/(?:claude-hooks\/hashes|sandbox\/fragment)\.mjs\s+--write$/u;

const roleOptions = (role) => {
  if (role === undefined || role === "main") {
    return {
      cwd: fixture.root,
      sessionCwd: fixture.root,
      modelName: "gemini-4-neon-xhigh",
      branch: "master",
    };
  }
  if (role === "implementer") {
    return {
      cwd: fixture.worktree,
      sessionCwd: fixture.worktree,
      modelName: "gemini-4-neon-xhigh",
      branch: "h-61",
    };
  }
  return {
    cwd: fixture.worktree,
    sessionCwd: fixture.worktree,
    modelName: "gemini-4-neon-high",
    agentType: role,
    branch: "h-61",
  };
};

test("Antigravity PreToolUse denies every Claude bash-cases denial without weakening", () => {
  const cases = readJson("harness/claude-hooks/bash-cases.json");
  const prelockExceptions = [];
  for (const item of cases) {
    const command = fill(item.command);
    const opts = roleOptions(item.role);
    const lock = item.lock ?? "off";
    const project = {
      ...projectFor(fixture, { lock }),
      ownedWorktree: opts.sessionCwd,
    };
    const claude = decideBash(
      {
        tool_name: "Bash",
        cwd: opts.cwd,
        ...roleInput(item.role),
        tool_input: { command },
      },
      project,
    );
    if (claude.decision !== DECISION.deny) continue;

    const antigravity = antigravityPreTool(
      {
        hookEventName: "PreToolUse",
        cwd: opts.cwd,
        modelName: opts.modelName,
        ...(opts.agentType === undefined ? {} : { agent_type: opts.agentType }),
        toolCall: {
          id: "parity-bash",
          name: "run_command",
          args: { CommandLine: command },
        },
      },
      project,
      opts.sessionCwd,
      { home: fakeHome, branch: opts.branch, threadId: undefined },
    );

    if (
      lock === "off" &&
      item.role === "implementer" &&
      PRELOCK_GENERATOR_CMD.test(command.trim())
    ) {
      prelockExceptions.push(command);
      assert.equal(antigravity.decision, "allow", command);
      const lockedProject = {
        ...projectFor(fixture, { lock: "on" }),
        ownedWorktree: opts.sessionCwd,
      };
      const lockedResult = antigravityPreTool(
        {
          hookEventName: "PreToolUse",
          cwd: opts.cwd,
          modelName: opts.modelName,
          toolCall: {
            id: "parity-bash-locked",
            name: "run_command",
            args: { CommandLine: command },
          },
        },
        lockedProject,
        opts.sessionCwd,
        { home: fakeHome, branch: opts.branch, threadId: undefined },
      );
      assert.equal(lockedResult.decision, "deny", `lock=on ${command}`);
      continue;
    }

    assert.equal(
      antigravity.decision,
      "deny",
      `Claude denied but Antigravity allowed (${item.role ?? "main"}, lock=${lock}): ${command} -> ${JSON.stringify(antigravity)}`,
    );
  }
  assert.ok(prelockExceptions.length >= 1);
});

test("Antigravity PreToolUse denies every Claude edit-cases denial without weakening", () => {
  const cases = readJson("harness/claude-hooks/edit-cases.json");
  let deniedCount = 0;
  for (const item of cases) {
    if (typeof item.path !== "string") continue;
    const rawPath = fill(item.path);
    const opts = roleOptions(item.role);
    const lock = item.lock ?? "off";
    const project = {
      ...projectFor(fixture, { lock }),
      ownedWorktree: opts.sessionCwd,
    };
    const claude = decideEdit(
      { ...roleInput(item.role), tool_input: { file_path: rawPath } },
      project,
    );
    if (claude.decision !== DECISION.deny) continue;

    deniedCount += 1;
    const antigravity = antigravityPreTool(
      {
        hookEventName: "PreToolUse",
        cwd: opts.cwd,
        modelName: opts.modelName,
        ...(opts.agentType === undefined ? {} : { agent_type: opts.agentType }),
        toolCall: {
          id: "parity-edit",
          name: "write_file",
          args: { Path: rawPath, Content: "x" },
        },
      },
      project,
      opts.sessionCwd,
      { home: fakeHome, branch: opts.branch, threadId: undefined },
    );

    if (
      lock === "off" &&
      item.role === "implementer" &&
      rawPath.startsWith(`${opts.sessionCwd}/`) &&
      PRELOCK_REVIEW_RECORD.test(rawPath)
    ) {
      assert.equal(antigravity.decision, "allow", rawPath);
      continue;
    }

    assert.equal(
      antigravity.decision,
      "deny",
      `Claude denied but Antigravity allowed (${item.role ?? "main"}, lock=${lock}): ${rawPath} -> ${JSON.stringify(antigravity)}`,
    );
  }
  assert.ok(deniedCount >= 50);

  const reviewInWorktree = join(fixture.worktree, "docs/reviews/H-61.md");
  const lockedProject = {
    ...projectFor(fixture, { lock: "on" }),
    ownedWorktree: fixture.worktree,
  };
  assert.equal(
    antigravityPreTool(
      {
        hookEventName: "PreToolUse",
        cwd: fixture.worktree,
        modelName: "gemini-4-neon-xhigh",
        toolCall: {
          id: "parity-review-locked",
          name: "write_file",
          args: { Path: reviewInWorktree, Content: "x" },
        },
      },
      lockedProject,
      fixture.worktree,
      { home: fakeHome, branch: "h-61", threadId: undefined },
    ).decision,
    "deny",
  );
});
