import { strict as assert } from "node:assert";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync, mkdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { configText, hooksConfig, setupText } from "../config.mjs";
import {
  createFixture,
  repoRoot,
  projectFor,
} from "../../claude-hooks/tests/fixture.mjs";
import { isWritable } from "../../../.claude/hooks/lib/allowlist.mjs";
import { hashProblems, currentHashes } from "../../claude-hooks/hashes.mjs";
const policy = JSON.parse(
  readFileSync(join(repoRoot, "harness/sandbox/policy.json"), "utf8"),
);
test("R5 repository config is independent of HOME and contains no machine identity", () => {
  assert.equal(
    configText(policy, "/a", "/node-a"),
    configText(policy, "/b", "/node-b"),
  );
  const run = spawnSync(process.execPath, ["harness/codex-hooks/config.mjs"], {
    cwd: repoRoot,
    env: { ...process.env, HOME: "/Users/runner" },
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  assert.doesNotMatch(configText(policy), /\/Users\//u);
});
test("R6 hook commands resolve the common git directory rather than the editable worktree", () => {
  for (const group of Object.values(hooksConfig().hooks))
    assert.match(group[0].hooks[0].command, /--git-common-dir/u);
});
test("R9 implementer instructions are protected", () => {
  const f = createFixture();
  try {
    assert.equal(
      isWritable(projectFor(f).allowlist, "docs/agents/implementer.md"),
      false,
    );
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
test("R14 new guard files cannot disappear with their hash entries", () => {
  const f = createFixture();
  try {
    const path = "harness/codex-hooks/added.mjs";
    mkdirSync(join(f.root, "harness/codex-hooks"), { recursive: true });
    writeFileSync(join(f.root, path), "export const value = 1;\n");
    assert.equal(spawnSync("git", ["add", path], { cwd: f.root }).status, 0);
    const hashes = currentHashes(f.root);
    delete hashes[path];
    assert.ok(
      hashProblems(f.root, hashes).some((value) => value.startsWith(path)),
    );
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
test("R14 hash folder coverage is fixed independently of the expected hashes", async () => {
  const { HASH_DIRECTORIES } = await import("../../claude-hooks/hashes.mjs");
  assert.deepEqual(HASH_DIRECTORIES, [
    ".claude/hooks",
    ".codex",
    "harness/codex-hooks",
    "harness/toolchain",
    "harness/codex-regression",
  ]);
});

test("Codex Seatbelt shell is recognized by the existing nested sandbox contract", () => {
  for (const [sandbox, inherited, expected] of [
    ["seatbelt", "", "1"],
    ["", "", ""],
    ["", "1", "1"],
  ]) {
    const run = spawnSync("just", ["--evaluate", "SANDBOX_RUNTIME"], {
      cwd: repoRoot,
      env: {
        ...process.env,
        CODEX_SANDBOX: sandbox,
        SANDBOX_RUNTIME: inherited,
      },
      encoding: "utf8",
    });
    assert.equal(run.status, 0, run.stderr);
    assert.equal(run.stdout.trim(), expected);
  }
});

test("Codex setup scopes test Unix sockets to the shared check temporary directory", () => {
  const f = createFixture();
  try {
    const text = setupText(
      f.worktree,
      "/example",
      "/node",
      "locked",
      "/runtime/temporary",
    );
    assert.ok(
      text.includes("[permissions.stock-copilot-session.network.unix_sockets]"),
    );
    assert.ok(text.includes('"/runtime/temporary" = "allow"'));
    assert.doesNotMatch(text, /dangerously_allow_all_unix_sockets/u);
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});

test("R4 R6 Git administrative identity and config files remain read only", () => {
  const f = createFixture();
  try {
    const admin = spawnSync("git", ["rev-parse", "--absolute-git-dir"], {
      cwd: f.worktree,
      encoding: "utf8",
    }).stdout.trim();
    const text = setupText(f.worktree);
    for (const name of ["commondir", "gitdir", "config.worktree"])
      assert.ok(
        text.includes(JSON.stringify(admin + "/" + name) + ' = "read"'),
      );
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});

test("R6 the permission profile covers lint test toolchain and main owned surfaces", () => {
  const text = configText(policy);
  for (const path of [
    "apps/desktop/eslint",
    "apps/desktop/eslint.config.mjs",
    "apps/desktop/tsconfig.json",
    "apps/desktop/tests",
    ".config",
    "package.json",
    "docs/progress.md",
  ])
    assert.ok(text.includes(JSON.stringify(path) + ' = "read"'), path);
});

test("R5 setup keeps project and offline fixture installs in the same pnpm store", () => {
  const f = createFixture();
  try {
    const text = setupText(f.worktree, "/example", "/node");
    assert.ok(
      text.includes('"pnpm_config_store_dir" = "/example/Library/pnpm/store"'),
    );
    const run = spawnSync("pnpm", ["config", "get", "store-dir"], {
      cwd: repoRoot,
      env: {
        ...process.env,
        pnpm_config_store_dir: "/example/Library/pnpm/store",
      },
      encoding: "utf8",
    });
    assert.equal(run.status, 0, run.stderr);
    assert.equal(run.stdout.trim(), "/example/Library/pnpm/store");
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
