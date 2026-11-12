import { strict as assert } from "node:assert";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  isWritable,
  parseAllowlist,
  patternMatches,
} from "../../../.claude/hooks/lib/allowlist.mjs";
import { readJson, realAllowlist, repoRoot } from "./fixture.mjs";

test("allowlist lines are narrow: whole segments, `**` only at the end, `!` excludes", () => {
  [
    "a/**/b\n",
    "**\n",
    "**/x\n",
    "a/*.ts\n",
    "a/b*\n",
    " a\n",
    "a \n",
    "\n",
    "a\n\nb\n",
    "a//b\n",
    "a/./b\n",
    "a/../b\n",
    "!\n",
    "a\na\n",
    "a",
    "a\\b\n",
  ].forEach((text) =>
    assert.notEqual(
      parseAllowlist(text).problem,
      undefined,
      JSON.stringify(text),
    ),
  );
  assert.deepEqual(parseAllowlist("a/*/b\nc/**\n!c/d\n"), {
    includes: ["a/*/b", "c/**"],
    excludes: ["c/d"],
  });
});

test("pattern matching is per segment and case-sensitive", () => {
  [
    ["docs/**", "docs/a.md", true],
    ["docs/**", "docs/adr/a.md", true],
    ["docs/**", "docs", false],
    ["docs/**", "Docs/a.md", false],
    ["crates/*/src/**", "crates/domain/src/lib.rs", true],
    ["crates/*/src/**", "crates/adapters/x/src/lib.rs", false],
    ["Cargo.lock", "Cargo.lock", true],
    ["Cargo.lock", "cargo.lock", false],
    ["a/*", "a/b/c", false],
  ].forEach(([pattern, path, expected]) =>
    assert.equal(patternMatches(pattern, path), expected, `${pattern} ${path}`),
  );
});

test("lock files and IPC generated files are writable", () => {
  const generated = readJson("harness/generated-files/files.json").files.map(
    (file) => file.path,
  );
  const responses = readdirSync(
    join(repoRoot, "apps/desktop/src-tauri/ipc/responses"),
  ).map((name) => `apps/desktop/src-tauri/ipc/responses/${name}`);
  assert.ok(responses.length > 0);
  [
    "Cargo.lock",
    "pnpm-lock.yaml",
    "apps/desktop/src-tauri/ipc/commands.txt",
    "apps/desktop/src-tauri/permissions/commands.toml",
    ...generated,
    ...responses,
  ].forEach((path) => assert.ok(isWritable(realAllowlist, path), path));
});

test("open-issues and every existing ADR are protected, a new ADR is not", () => {
  const adrs = readdirSync(join(repoRoot, "docs/adr")).map(
    (name) => `docs/adr/${name}`,
  );
  assert.ok(adrs.length >= 18);
  [
    "docs/open-issues.md",
    "docs/harness.md",
    "docs/workflow.md",
    ...adrs,
  ].forEach((path) => assert.ok(!isWritable(realAllowlist, path), path));
  assert.ok(isWritable(realAllowlist, "docs/adr/0099-future.md"));
});

test("harness 7.1 protected files are outside the allowlist", () => {
  [
    ".claude/settings.json",
    ".claude/settings.local.json",
    ".claude/hooks/guard-edit.mjs",
    ".claude/hooks/lib/bash-guard.mjs",
    ".claude/agents/implementer.md",
    ".claude/skills/x/SKILL.md",
    ".claude/commands/x.md",
    ".mcp.json",
    ".worktreeinclude",
    "harness/agent-writable.txt",
    "harness/claude-hooks/lock.txt",
    "harness/deny/x.toml",
    "crates/xtask/src/main.rs",
    "crates/xtask/Cargo.toml",
    "justfile",
    "lefthook.yml",
    "lefthook-local.yml",
    ".lefthook-local.yml",
    ".github/workflows/ci.yml",
    "apps/desktop/eslint.config.mjs",
    "apps/desktop/eslint.scripts.config.mjs",
    "apps/desktop/eslint/rules/no-comments.mjs",
    "apps/desktop/tsconfig.json",
    "apps/desktop/vitest.config.mjs",
    "apps/desktop/knip.json",
    "deny.toml",
    "clippy.toml",
    "rustfmt.toml",
    "rust-toolchain.toml",
    ".config/nextest.toml",
    ".cargo/config.toml",
    ".cargo/mutants.toml",
    "AGENTS.md",
    "CLAUDE.md",
    "design/tokens.json",
    "crates/adapters/store-sqlite/migrations.lock",
    "apps/desktop/src-tauri/capabilities/main.json",
    "apps/desktop/src-tauri/tauri.conf.json",
    "apps/desktop/src-tauri/build.rs",
    "crates/trade-fakes/src/lib.rs",
    "crates/adapters/broker-toss-schema/src/lib.rs",
    "apps/desktop/src/shared/ds/index.ts",
    "apps/desktop/src/shared/ds/tokens.css",
    "apps/desktop/src/shared/ds/tokens.ts",
    "package.json",
    ".gitignore",
  ].forEach((path) => assert.ok(!isWritable(realAllowlist, path), path));
});
