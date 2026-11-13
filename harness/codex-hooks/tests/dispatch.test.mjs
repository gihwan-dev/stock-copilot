import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { chmodSync, cpSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { createFixture, repoRoot } from "../../claude-hooks/tests/fixture.mjs";

const runFixture = (verify) => {
  const f = createFixture();
  try {
    for (const path of [
      ".claude/hooks",
      "harness/claude-hooks",
      "harness/codex-hooks",
      "harness/agent-writable.txt",
    ])
      cpSync(join(repoRoot, path), join(f.root, path), { recursive: true });
    const bin = join(f.base, "bin");
    mkdirSync(bin);
    for (const name of ["cargo", "pnpm", "just"]) {
      const file = join(bin, name);
      writeFileSync(
        file,
        name === "just"
          ? '#!/bin/sh\nprintf "probe-just %s\\n" "$*"\nexit 1\n'
          : "#!/bin/sh\nexit 0\n",
      );
      chmodSync(file, 0o700);
    }
    const run = (input) => {
      const result = spawnSync(
        process.execPath,
        [join(f.root, "harness/codex-hooks/dispatch.mjs")],
        {
          input: JSON.stringify({ cwd: f.worktree, ...input }),
          encoding: "utf8",
          env: { ...process.env, PATH: `${bin}:${process.env.PATH}` },
        },
      );
      assert.equal(result.status, 0, result.stderr);
      return result.stdout.trim() === ""
        ? undefined
        : JSON.parse(result.stdout);
    };
    verify(run, f);
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
};

test("Codex post-edit runs lint-file inside the shared check sandbox and returns its failure", () => {
  runFixture((run, f) => {
    const output = run({
      hook_event_name: "PostToolUse",
      tool_name: "apply_patch",
      tool_input: {
        command: `*** Begin Patch\n*** Update File: ${f.worktree}/crates/domain/src/lib.rs\n@@\n-x\n+y\n*** End Patch`,
      },
    });
    assert.equal(output.decision, "block");
    assert.match(output.reason, /probe-just lint-file/u);
  });
});

test("Codex stop runs offline fresh steps and the full check then permits only a blocked report", () => {
  runFixture((run) => {
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
    const blocked = run({
      hook_event_name: "Stop",
      last_assistant_message: "막힘: 검사 실패",
      stop_hook_active: true,
    });
    assert.match(blocked.systemMessage, /전체 검사 실패/u);
  });
});
