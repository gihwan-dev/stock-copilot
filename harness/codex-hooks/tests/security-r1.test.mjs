import { strict as assert } from "node:assert";
import { test, after } from "node:test";
import { rmSync, symlinkSync } from "node:fs";
import { codexPreTool, patchPaths } from "../adapter.mjs";
import {
  createFixture,
  projectFor,
} from "../../claude-hooks/tests/fixture.mjs";
const f = createFixture();
after(() => rmSync(f.base, { recursive: true, force: true }));
const deny = (input) =>
  codexPreTool(
    { cwd: f.worktree, ...input },
    projectFor(f, { lock: "on" }),
    f.worktree,
  )?.hookSpecificOutput?.permissionDecision;
const patch = (header) =>
  `*** Begin Patch\n${header}\n@@\n-x\n+y\n*** End Patch`;
test("R1 indented CRLF markers cannot hide protected patch targets", () => {
  assert.deepEqual(
    patchPaths(patch(" *** Update File: justfile").replaceAll("\n", "\r\n")),
    ["justfile"],
  );
  assert.equal(
    deny({
      tool_name: "apply_patch",
      tool_input: { command: patch(" *** Update File: justfile") },
    }),
    "deny",
  );
  for (const text of [
    patch("*** Unknown: x"),
    "prefix\n" + patch("*** Update File: x"),
    patch("*** Update File: x") + "\n" + patch("*** Update File: y"),
  ])
    assert.throws(() => patchPaths(text));
});
test("R2 whitespace and controls in paths are refused before normalization", () => {
  for (const path of [
    "docs/harness.md ",
    "docs/harness.md\t",
    "docs/a b.md",
    "docs/a\u0000.md",
  ])
    assert.equal(
      deny({
        tool_name: "apply_patch",
        tool_input: { command: patch(`*** Update File: ${path}`) },
      }),
      "deny",
    );
});
test("R4 session role and edit scope cannot be changed by a tool cwd or absolute target", () => {
  assert.equal(
    deny({ tool_name: "Bash", cwd: f.root, tool_input: { command: "pwd" } }),
    "deny",
  );
  assert.equal(
    deny({
      tool_name: "apply_patch",
      tool_input: {
        command: patch(`*** Update File: ${f.root}/docs/notes.md`),
      },
    }),
    "deny",
  );
});
test("R7 Bash heredoc patches share the patch guard and dynamic patch input is denied", () => {
  for (const command of [
    `apply_patch <<\u0027EOF\u0027\n${patch(" *** Update File: justfile")}\nEOF`,
    "applypatch < patch.txt",
    'apply_patch "$PATCH"',
    'node -e "require(\u0027child_process\u0027).execSync(\u0027apply_patch\u0027)"',
  ])
    assert.equal(deny({ tool_name: "Bash", tool_input: { command } }), "deny");
});
test("R13 unknown executable tools are denied and R16 either subagent field denies a benign command", () => {
  assert.equal(deny({ tool_name: "unknown_executor", tool_input: {} }), "deny");
  for (const field of ["agent_id", "agent_type"])
    assert.equal(
      deny({
        tool_name: "Bash",
        [field]: "nested",
        tool_input: { command: "pwd" },
      }),
      "deny",
    );
});
test("R15 nested guidance files cannot replace the protected instructions", () => {
  for (const path of [
    "docs/AGENTS.md",
    "AGENTS.override.md",
    "docs/AGENTS.override.md",
  ])
    assert.equal(
      deny({
        tool_name: "apply_patch",
        tool_input: { command: patch("*** Add File: " + path) },
      }),
      "deny",
    );
});

test("R4 equivalent filesystem cwd aliases keep the session role and scope", () => {
  const alias = f.base + "/alias";
  symlinkSync(f.worktree, alias);
  assert.notEqual(
    deny({ tool_name: "Bash", cwd: alias, tool_input: { command: "pwd" } }),
    "deny",
  );
});

test("R7 patch cwd cannot be changed by shell compounds or wrappers", () => {
  assert.notEqual(
    deny({
      tool_name: "Bash",
      tool_input: {
        command:
          "apply_patch <<" +
          String.fromCharCode(39) +
          "EOF" +
          String.fromCharCode(39) +
          "\n*** Begin Patch\n*** Add File: crates/domain/src/probe.rs\n+x\n*** End Patch\nEOF",
      },
    }),
    "deny",
  );
  for (const prefix of ["cd " + f.root + " && ", "env -C " + f.root + " "]) {
    const command =
      prefix +
      "apply_patch <<" +
      String.fromCharCode(39) +
      "EOF" +
      String.fromCharCode(39) +
      "\n" +
      patch("*** Add File: crates/domain/src/probe.rs") +
      "\nEOF";
    assert.equal(deny({ tool_name: "Bash", tool_input: { command } }), "deny");
  }
});

test("R2 lexical path normalization preserves allowed edits and guards protected targets", () => {
  for (const path of [
    "./crates/domain/src/probe.rs",
    "crates/domain/src/nested/../probe.rs",
  ])
    assert.notEqual(
      deny({
        tool_name: "apply_patch",
        tool_input: { command: patch("*** Update File: " + path) },
      }),
      "deny",
    );
  assert.equal(
    deny({
      tool_name: "apply_patch",
      tool_input: { command: patch("*** Update File: crates/../justfile") },
    }),
    "deny",
  );
});

test("R7 quoted concatenated escaped and case variants cannot hide patch executables", () => {
  const quote = String.fromCharCode(39);
  for (const name of [
    "apply_" + quote + quote + "patch",
    "apply" + String.fromCharCode(92) + "_patch",
    "APPLYPATCH",
  ]) {
    const command =
      name +
      " <<" +
      quote +
      "EOF" +
      quote +
      "\n" +
      patch("*** Update File: justfile") +
      "\nEOF";
    assert.equal(
      deny({ tool_name: "Bash", tool_input: { command } }),
      "deny",
      name,
    );
  }
});
