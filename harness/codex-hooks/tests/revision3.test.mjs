import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  cpSync,
  readFileSync,
  realpathSync,
  rmSync,
  mkdtempSync,
  symlinkSync,
  chmodSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { codexPreTool } from "../adapter.mjs";
import { setupText } from "../config.mjs";
import { dispatchEvent } from "../dispatch.mjs";
import { sessionProject } from "../session.mjs";
import { sessionTemporary, prepareDirectory } from "../temporary.mjs";
import {
  regressionEnvironment,
  regressionPermissions,
} from "../../codex-regression/run.mjs";
import { orchestrationFindings } from "../../../.claude/hooks/lib/orchestration.mjs";
import {
  createFixture,
  projectFor,
  repoRoot,
} from "../../claude-hooks/tests/fixture.mjs";

const fields = [
  "workdir",
  "cwd",
  "working_directory",
  "workingDirectory",
  "working_dir",
  "workingDir",
  "work_dir",
  "workDir",
  "directory",
];
test("R3 shell workdir fields cannot escape or override the directory used by the guard", () => {
  const f = createFixture();
  try {
    const project = projectFor(f, { lock: "on" });
    const decision = (tool_input, tool_name = "Bash") =>
      codexPreTool(
        { tool_name, cwd: f.worktree, tool_input },
        project,
        f.worktree,
      )?.hookSpecificOutput?.permissionDecision;
    for (const name of ["Bash", "exec_command", "shell_command", "shell"]) {
      for (const field of fields) {
        for (const value of [
          f.root,
          "",
          null,
          1,
          {},
          f.worktree + "/../../outside",
        ])
          assert.equal(
            decision({ command: "pwd", [field]: value }, name),
            "deny",
            name + " " + field,
          );
        assert.notEqual(
          decision({ command: "touch notes.md", [field]: "docs" }, name),
          "deny",
          name + " " + field,
        );
      }
      assert.equal(
        decision(
          { command: "pwd", action: { working_directory: f.root } },
          name,
        ),
        "deny",
      );
      assert.equal(
        decision(
          { command: "pwd", workdir: f.worktree, cwd: f.worktree + "/docs" },
          name,
        ),
        "deny",
      );
    }
    symlinkSync(f.root, join(f.worktree, "docs/outside"));
    assert.equal(decision({ command: "pwd", workdir: "docs/outside" }), "deny");
    assert.equal(decision({ command: "pwd" }, "local_shell"), "deny");
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
test("R3 remote and local launch branches must match a valid brief branch", () => {
  const rules = JSON.parse(
    readFileSync(
      join(repoRoot, "harness/claude-hooks/agent-rules.json"),
      "utf8",
    ),
  );
  const base = "a".repeat(40);
  const facts = {
    baseTip: base,
    originTip: base,
    resolveCommit: (value) => value,
    passedTargets: new Map(),
    worktreeForBranch: () => "/topic",
  };
  for (const remote of [true, false]) {
    const make = (branch, briefBranch = "topic") => ({
      tool_name: "mcp__t3_code__t3_thread_launch",
      tool_input: {
        message:
          "- 기준 커밋: " +
          base +
          ", 브랜치 " +
          String.fromCharCode(96) +
          briefBranch +
          String.fromCharCode(96),
        modelSelection: { provider: "codex", model: "gpt-6.1-sol" },
        workspaceStrategy: {
          type: "worktree",
          branch,
          baseRef: remote ? "origin/master" : base,
          startFromOrigin: remote,
        },
      },
    });
    assert.deepEqual(orchestrationFindings(make("topic"), rules, facts), []);
    for (const branch of ["other", undefined, 1, null])
      assert.ok(
        orchestrationFindings(make(branch), rules, facts).length,
        String(branch),
      );
    for (const branch of ["master", "bad branch", "-unsafe"])
      assert.ok(
        orchestrationFindings(make(branch, branch), rules, facts).length,
        branch,
      );
  }
});
test("R3 setup and hook checks share a dedicated temporary directory rather than the user temp root", () => {
  const f = createFixture();
  try {
    for (const path of ["harness/claude-hooks", "harness/agent-writable.txt"])
      cpSync(join(repoRoot, path), join(f.root, path), { recursive: true });
    const text = setupText(f.worktree);
    assert.ok(
      !text.includes(JSON.stringify(realpathSync(tmpdir())) + ' = "allow"'),
    );
    const temporary = JSON.parse(/^"TMPDIR" = (.+)$/mu.exec(text)[1]);
    assert.ok(text.includes(JSON.stringify(temporary) + ' = "allow"'));
    assert.ok(text.includes(JSON.stringify(temporary) + ' = "write"'));
    let places;
    dispatchEvent(
      { hook_event_name: "Stop", cwd: f.worktree },
      sessionProject(f.worktree),
      {
        check: (value) => {
          places = value;
          return { status: 0, output: "" };
        },
        lint: () => ({ status: 0, output: "" }),
      },
      () => {},
    );
    assert.equal(places.temporary, temporary);
    assert.notEqual(temporary, sessionTemporary(f.root));
    assert.equal(setupText(join(f.worktree, "docs")), text);
    let linted;
    dispatchEvent(
      {
        hook_event_name: "PostToolUse",
        tool_name: "Bash",
        cwd: f.worktree,
        tool_input: {
          workdir: "crates",
          command:
            "apply_patch <<'EOF'\n*** Begin Patch\n*** Update File: domain/src/lib.rs\n@@\n-x\n+y\n*** End Patch\nEOF",
        },
      },
      sessionProject(f.worktree),
      {
        check: () => ({ status: 0, output: "" }),
        lint: (_places, _worktree, command) => {
          linted = command[2];
          return { status: 0, output: "" };
        },
      },
      () => {},
    );
    assert.equal(linted, join(f.worktree, "crates/domain/src/lib.rs"));
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
test("R3 regression refuses ambient Codex homes and accepts only private isolated temporary homes", () => {
  const home = mkdtempSync(join(tmpdir(), "h41-r3-home-"));
  try {
    assert.throws(() => regressionEnvironment({}), /CODEX_HOME/u);
    assert.throws(
      () => regressionEnvironment({ CODEX_HOME: "relative" }),
      /CODEX_HOME/u,
    );
    assert.equal(
      regressionEnvironment({ CODEX_HOME: home }).CODEX_HOME,
      realpathSync(home),
    );
    assert.deepEqual(
      regressionPermissions({ CODEX_HOME: home }, join(home, "state")),
      { [home]: "deny", [join(home, "state")]: "deny" },
    );
    chmodSync(home, 0o755);
    assert.throws(
      () => regressionEnvironment({ CODEX_HOME: home }),
      /CODEX_HOME/u,
    );
    chmodSync(home, 0o700);
    symlinkSync(home, join(home, "linked"));
    assert.throws(
      () => regressionEnvironment({ CODEX_HOME: join(home, "linked") }),
      /CODEX_HOME/u,
    );
  } finally {
    rmSync(home, { recursive: true, force: true });
  }
});
test("R3 check temporary folders are private and reject symlink substitution", () => {
  const f = createFixture();
  const temporary = join(f.base, "private");
  try {
    assert.equal(prepareDirectory(temporary), temporary);
    chmodSync(temporary, 0o755);
    assert.throws(() => prepareDirectory(temporary), /권한/u);
    rmSync(temporary, { recursive: true });
    symlinkSync(f.base, temporary);
    assert.throws(() => prepareDirectory(temporary), /권한/u);
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
