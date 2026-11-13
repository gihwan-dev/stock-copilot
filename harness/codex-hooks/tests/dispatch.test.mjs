import { strict as assert } from "node:assert";
import { test } from "node:test";
import { cpSync, rmSync } from "node:fs";
import { join } from "node:path";
import { dispatchEvent } from "../dispatch.mjs";
import { sessionProject } from "../session.mjs";
import { checkProfile } from "../../../.claude/hooks/lib/check-sandbox.mjs";
import { createFixture, repoRoot } from "../../claude-hooks/tests/fixture.mjs";
const fixture = (verify, status = 1) => {
  const f = createFixture();
  try {
    for (const path of ["harness/claude-hooks", "harness/agent-writable.txt"])
      cpSync(join(repoRoot, path), join(f.root, path), { recursive: true });
    const session = sessionProject(f.worktree);
    const records = [];
    const calls = [];
    const run = (input) =>
      dispatchEvent(
        { cwd: f.worktree, ...input },
        session,
        {
          lint: (places, worktree, command) => {
            calls.push({ places, worktree, command });
            return {
              status,
              output: "probe-just " + command.slice(1).join(" "),
            };
          },
          check: (places, worktree, base) => {
            calls.push({ places, worktree, base });
            return {
              status,
              output:
                "cargo clean\npnpm install\nprobe-just base=" + base + " check",
            };
          },
        },
        (...args) => records.push(args.at(-1)),
      );
    verify(run, f, calls, records);
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
};
test("Codex post-edit runs lint-file inside the shared check sandbox and returns its failure", () => {
  fixture((run, f, calls) => {
    const output = run({
      hook_event_name: "PostToolUse",
      tool_name: "apply_patch",
      tool_input: {
        command:
          "*** Begin Patch\n*** Update File: " +
          f.worktree +
          "/crates/domain/src/lib.rs\n@@\n-x\n+y\n*** End Patch",
      },
    });
    assert.equal(output.decision, "block");
    assert.match(output.reason, /probe-just lint-file/u);
    assert.deepEqual(calls[0].command, [
      "just",
      "lint-file",
      f.worktree + "/crates/domain/src/lib.rs",
    ]);
    assert.equal(calls[0].worktree, f.worktree);
    assert.match(checkProfile(calls[0].places), /deny network-outbound/u);
  });
});
test("Codex stop runs offline fresh steps and the full check then permits only a blocked report", () => {
  fixture((run, f, calls) => {
    const output = run({
      hook_event_name: "Stop",
      last_assistant_message: "완료",
      stop_hook_active: false,
    });
    assert.equal(output.decision, "block");
    assert.match(
      output.reason,
      /cargo clean.*pnpm install.*probe-just base=[a-f0-9]{40} check/su,
    );
    assert.equal(calls[0].worktree, f.worktree);
    assert.match(checkProfile(calls[0].places), /deny network-outbound/u);
    assert.match(
      run({
        hook_event_name: "Stop",
        last_assistant_message: "막힘: 검사 실패",
        stop_hook_active: true,
      }).systemMessage,
      /전체 검사 실패/u,
    );
  });
});
test("R10 a successful clean Codex stop fills the existing passed target registry", () => {
  fixture((run, f, calls, records) => {
    run({
      hook_event_name: "Stop",
      session_id: "session",
      last_assistant_message: "완료",
    });
    assert.equal(records.length, 1);
    assert.equal(records[0].status, "passed");
    assert.match(records[0].head, /^[a-f0-9]{40}$/u);
  }, 0);
});

test("R7 post-edit lints the targets of a Bash heredoc patch", () => {
  fixture((run, f, calls) => {
    const output = run({
      hook_event_name: "PostToolUse",
      tool_name: "Bash",
      tool_input: {
        command:
          "apply_patch <<" +
          String.fromCharCode(39) +
          "EOF" +
          String.fromCharCode(39) +
          "\n*** Begin Patch\n*** Add File: crates/domain/src/lib.rs\n+x\n*** End Patch\nEOF",
      },
    });
    assert.equal(output?.decision, "block");
    assert.equal(calls[0].command[1], "lint-file");
    assert.equal(calls[0].worktree, f.worktree);
  });
});
