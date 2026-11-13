import { strict as assert } from "node:assert";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { patchPaths, codexPreTool } from "../adapter.mjs";
import { decideBash } from "../../../.claude/hooks/lib/bash-guard.mjs";
import {
  createFixture,
  projectFor,
} from "../../claude-hooks/tests/fixture.mjs";

const patch = (header) =>
  "*** Begin Patch\n" + header + "\n@@\n-x\n+y\n*** End Patch";
test("R2 patch classification rejects every Rust whitespace header disguise and unknown body line", () => {
  const f = createFixture();
  try {
    const source = join(f.base, "whitespace.rs");
    const executable = join(f.base, "whitespace");
    writeFileSync(
      source,
      'fn main() { for n in 0..=0x10ffff { if let Some(c) = char::from_u32(n) { if c.is_whitespace() { println!("{:x}", n); } } } }',
    );
    const built = spawnSync("rustc", [source, "-o", executable], {
      encoding: "utf8",
    });
    assert.equal(built.status, 0, built.stderr);
    const run = spawnSync(executable, [], { encoding: "utf8" });
    assert.equal(run.status, 0, run.stderr);
    const characters = run.stdout
      .trim()
      .split("\n")
      .map((value) => String.fromCodePoint(Number.parseInt(value, 16)));
    assert.ok(characters.includes(String.fromCodePoint(0x85)));
    const project = projectFor(f, { lock: "on" });
    for (const character of [...characters, String.fromCodePoint(0xfeff)]) {
      const text = patch(character + "*** Update File: justfile").replace(
        "*** End Patch",
        "*** Update File: crates/domain/src/lib.rs\n@@\n-x\n+y\n*** End Patch",
      );
      for (const input of [
        { tool_name: "apply_patch", tool_input: { command: text } },
        {
          tool_name: "Bash",
          tool_input: { command: "apply_patch <<'EOF'\n" + text + "\nEOF" },
        },
      ])
        assert.equal(
          codexPreTool({ cwd: f.worktree, ...input }, project, f.worktree)
            ?.hookSpecificOutput?.permissionDecision,
          "deny",
          character.codePointAt(0).toString(16),
        );
    }
    for (const line of [
      "ignored body",
      "unexpected",
      "\u0085*** Update File: justfile",
      "\ufeff*** Update File: justfile",
      " \u0085*** Update File: justfile",
    ])
      assert.throws(
        () =>
          patchPaths(
            patch("*** Update File: crates/domain/src/lib.rs").replace(
              "@@",
              line + "\n@@",
            ),
          ),
        line,
      );
    assert.deepEqual(
      patchPaths(
        "*** Begin Patch\n*** Add File: docs/new.md\n+한글 *** 내용\n*** End Patch",
      ),
      ["docs/new.md"],
    );
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
test("R2 shared shell core denies opaque writers and find side effects for both adapters", () => {
  const f = createFixture();
  try {
    const project = projectFor(f, { lock: "off" });
    const commands = [
      'codex --codex-run-as-apply-patch "$(cat p.txt)"',
      "cat p.txt | xargs -0 codex --codex-run-as-apply-patch",
      "codex apply",
      "codex --help",
      "git apply p.diff",
      "git am p.diff",
      "patch -p1 < p.diff",
      "xargs touch",
      "echo x | xargs -I{} sh -c '{}'",
      "find . -delete",
      "find . -exec echo ok \\;",
      "find . -execdir echo ok \\;",
      "find . -ok echo ok \\;",
      ...["-fprint", "-fprint0", "-fprintf", "-fls"].map(
        (flag) => "find . -maxdepth 0 " + flag + " /outside/HEAD",
      ),
    ];
    for (const command of commands) {
      const input = {
        tool_name: "Bash",
        cwd: f.worktree,
        agent_id: "implementer",
        agent_type: "implementer",
        tool_input: { command },
      };
      assert.equal(decideBash(input, project).decision, "deny", command);
      const { agent_id, agent_type, ...codex } = input;
      assert.equal(
        codexPreTool(codex, project, f.worktree)?.hookSpecificOutput
          ?.permissionDecision,
        "deny",
        command,
      );
    }
    assert.equal(
      decideBash(
        {
          cwd: f.worktree,
          tool_input: { command: "find . -name '*.rs' -print" },
        },
        project,
      ).decision,
      "allow",
    );
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
