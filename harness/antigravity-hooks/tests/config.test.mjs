import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  HOOKS_FILE,
  eventHookCommand,
  hooksConfig,
  hooksText,
} from "../config.mjs";
import { isWritable } from "../../../.claude/hooks/lib/allowlist.mjs";
import { loadProject } from "../../../.claude/hooks/lib/project.mjs";
import {
  ANTIGRAVITY_HASH_DIRECTORIES,
  HASHES_FILE,
  currentHashes,
  hashProblems,
} from "../../claude-hooks/hashes.mjs";
import { mkdtempSync } from "../../ci/test-temp.mjs";
import { createFixture, repoRoot } from "../../claude-hooks/tests/fixture.mjs";

test("Antigravity hooks config covers pre post stop resolves common git dir and fails closed", () => {
  const config = hooksConfig();
  const rootHook = config["stock-copilot-harness"];
  assert.deepEqual(Object.keys(rootHook), [
    "PreToolUse",
    "PostToolUse",
    "Stop",
  ]);
  assert.equal(rootHook.PreToolUse[0].matcher, "*");
  assert.equal(rootHook.PostToolUse[0].matcher, "*");
  assert.ok(Array.isArray(rootHook.Stop));
  assert.equal(rootHook.Stop[0].type, "command");
  assert.doesNotMatch(hooksText(), /\/Users\//u);

  for (const event of ["PreToolUse", "PostToolUse", "Stop"]) {
    const command = eventHookCommand(event);
    assert.match(command, /--git-common-dir/u);
    assert.match(
      command,
      /cat-file -e HEAD:harness\/antigravity-hooks\/dispatch\.mjs/u,
    );
  }

  const scratch = mkdtempSync(join(tmpdir(), "ag-hook-failclosed-"));
  try {
    const brokenPre = spawnSync(
      "/bin/sh",
      ["-c", eventHookCommand("PreToolUse")],
      {
        cwd: scratch,
        input: "{}",
        encoding: "utf8",
        env: { PATH: "/usr/bin:/bin" },
      },
    );
    assert.equal(brokenPre.status, 0);
    assert.equal(JSON.parse(brokenPre.stdout).decision, "deny");

    const brokenStop = spawnSync("/bin/sh", ["-c", eventHookCommand("Stop")], {
      cwd: scratch,
      input: "{}",
      encoding: "utf8",
      env: { PATH: "/usr/bin:/bin" },
    });
    assert.equal(brokenStop.status, 0);
    assert.equal(JSON.parse(brokenStop.stdout).decision, "continue");

    const brokenPost = spawnSync(
      "/bin/sh",
      ["-c", eventHookCommand("PostToolUse")],
      {
        cwd: scratch,
        input: "{}",
        encoding: "utf8",
        env: { PATH: "/usr/bin:/bin" },
      },
    );
    assert.equal(brokenPost.status, 0);
    assert.deepEqual(JSON.parse(brokenPost.stdout), {});
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
});

test("committed Antigravity hooks file matches generator and is protected and hashed", () => {
  const actual = readFileSync(join(repoRoot, HOOKS_FILE), "utf8");
  assert.equal(actual, hooksText());

  const check = spawnSync(
    process.execPath,
    [join(repoRoot, "harness/antigravity-hooks/config.mjs")],
    { cwd: repoRoot, encoding: "utf8" },
  );
  assert.equal(check.status, 0, check.stderr);

  const project = loadProject(repoRoot);
  for (const path of [
    HOOKS_FILE,
    "harness/antigravity-hooks/adapter.mjs",
    "harness/antigravity-hooks/dispatch.mjs",
    "harness/antigravity-hooks/policy.mjs",
    "harness/antigravity-hooks/config.mjs",
  ]) {
    assert.equal(isWritable(project.allowlist, path), false, path);
  }

  assert.deepEqual(ANTIGRAVITY_HASH_DIRECTORIES, [
    ".agents",
    "harness/antigravity-hooks",
  ]);
  const expectedHashes = JSON.parse(
    readFileSync(join(repoRoot, HASHES_FILE), "utf8"),
  );
  assert.ok(Object.hasOwn(expectedHashes, HOOKS_FILE));
  assert.ok(
    Object.hasOwn(expectedHashes, "harness/antigravity-hooks/adapter.mjs"),
  );
  assert.deepEqual(hashProblems(repoRoot, expectedHashes), []);
});

test("new files under .agents or harness/antigravity-hooks cannot disappear with their hashes", () => {
  const f = createFixture();
  try {
    const added = "harness/antigravity-hooks/extra.mjs";
    mkdirSync(join(f.root, "harness/antigravity-hooks"), { recursive: true });
    writeFileSync(join(f.root, added), "export const x = 1;\n");
    assert.equal(spawnSync("git", ["add", added], { cwd: f.root }).status, 0);
    const hashes = currentHashes(f.root);
    delete hashes[added];
    assert.ok(
      hashProblems(f.root, hashes).some((line) => line.startsWith(added)),
    );
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});

test("wrapper prefers main checkout dispatch after merge and fails closed if tampered", () => {
  const f = createFixture();
  try {
    const mainDispatch = join(f.root, "harness/antigravity-hooks/dispatch.mjs");
    const wtDispatch = join(
      f.worktree,
      "harness/antigravity-hooks/dispatch.mjs",
    );
    const wtAgents = join(f.worktree, ".agents");
    mkdirSync(join(f.root, "harness/antigravity-hooks"), { recursive: true });
    mkdirSync(join(f.worktree, "harness/antigravity-hooks"), {
      recursive: true,
    });
    mkdirSync(wtAgents, { recursive: true });
    writeFileSync(
      mainDispatch,
      'process.stdout.write(JSON.stringify({ decision: "deny", reason: "from-main" }) + "\\n");\n',
    );
    writeFileSync(
      wtDispatch,
      'process.stdout.write(JSON.stringify({ decision: "allow", reason: "from-wt" }) + "\\n");\n',
    );
    assert.equal(
      spawnSync("git", ["add", "harness/antigravity-hooks/dispatch.mjs"], {
        cwd: f.root,
      }).status,
      0,
    );
    assert.equal(
      spawnSync(
        "git",
        [
          "-c",
          "user.name=t",
          "-c",
          "user.email=t@e.invalid",
          "-c",
          "commit.gpgsign=false",
          "commit",
          "-q",
          "-m",
          "add main dispatch",
        ],
        { cwd: f.root },
      ).status,
      0,
    );

    const preferMain = spawnSync(
      "/bin/sh",
      ["-c", eventHookCommand("PreToolUse")],
      {
        cwd: wtAgents,
        input: "{}",
        encoding: "utf8",
      },
    );
    assert.equal(preferMain.status, 0, preferMain.stderr);
    assert.equal(JSON.parse(preferMain.stdout).reason, "from-main");

    rmSync(mainDispatch);
    const deletedMainFailsClosed = spawnSync(
      "/bin/sh",
      ["-c", eventHookCommand("PreToolUse")],
      {
        cwd: wtAgents,
        input: "{}",
        encoding: "utf8",
      },
    );
    assert.equal(deletedMainFailsClosed.status, 0);
    assert.equal(JSON.parse(deletedMainFailsClosed.stdout).decision, "deny");
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
