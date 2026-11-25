import { strict as assert } from "node:assert";
import { execFileSync } from "node:child_process";
import { cpSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { after, test } from "node:test";
import { antigravityPreTool, antigravityStop } from "../adapter.mjs";
import {
  BLOCKED_FILE,
  blockedExit,
  gitDirectory,
  recordBlocked,
  syncMaster,
} from "../staged-sync.mjs";
import { mkdtempSync } from "../../ci/test-temp.mjs";
import {
  createFixture,
  projectFor,
  repoRoot,
} from "../../claude-hooks/tests/fixture.mjs";
import { LOCK } from "../../../.claude/hooks/lib/project.mjs";

const scratch = mkdtempSync("staged-sync-");
after(() => rmSync(scratch, { recursive: true, force: true }));

const git = (cwd, ...args) =>
  execFileSync(
    "git",
    [
      "-C",
      cwd,
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.com",
      "-c",
      "commit.gpgsign=false",
      "-c",
      "core.hooksPath=/dev/null",
      ...args,
    ],
    { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
  ).trim();

let counter = 0;
const repository = () => {
  counter += 1;
  const root = join(scratch, `repo-${counter}`);
  mkdirSync(root, { recursive: true });
  git(root, "init", "-q", "-b", "master");
  writeFileSync(join(root, "a.txt"), "a\n");
  writeFileSync(join(root, "b.txt"), "b\n");
  git(root, "add", "a.txt", "b.txt");
  git(root, "commit", "-q", "-m", "base");
  git(root, "switch", "-q", "-c", "work");
  return root;
};

const advanceMaster = (root, file, content) => {
  git(root, "switch", "-q", "master");
  writeFileSync(join(root, file), content);
  git(root, "add", file);
  git(root, "commit", "-q", "-m", `master ${file}`);
  git(root, "update-ref", "refs/remotes/origin/master", "HEAD");
  git(root, "switch", "-q", "work");
};

const stage = (root, file, content) => {
  writeFileSync(join(root, file), content);
  git(root, "add", file);
};

test("sync merges origin/master and restores the staged change", () => {
  const root = repository();
  advanceMaster(root, "m.txt", "m\n");
  stage(root, "a.txt", "staged\n");
  const before = git(root, "rev-parse", "HEAD");
  const outcome = syncMaster(root);
  assert.equal(outcome.ok, true, outcome.message);
  assert.equal(git(root, "rev-parse", "HEAD^1"), before);
  assert.equal(
    git(root, "rev-parse", "HEAD^2"),
    git(root, "rev-parse", "origin/master"),
  );
  assert.equal(git(root, "diff", "--cached", "--name-only"), "a.txt");
  assert.equal(git(root, "diff", "--name-only"), "");
  assert.equal(git(root, "show", ":a.txt"), "staged");
});

test("sync rolls back when the staged change overlaps master", () => {
  const root = repository();
  advanceMaster(root, "a.txt", "master\n");
  stage(root, "a.txt", "staged\n");
  const before = git(root, "rev-parse", "HEAD");
  const outcome = syncMaster(root);
  assert.equal(outcome.ok, false);
  assert.equal(git(root, "rev-parse", "HEAD"), before);
  assert.equal(git(root, "show", ":a.txt"), "staged");
  assert.equal(git(root, "diff", "--name-only"), "");
});

test("sync rolls back a conflicting merge and keeps the staged change", () => {
  const root = repository();
  writeFileSync(join(root, "b.txt"), "work\n");
  git(root, "commit", "-q", "-am", "work b");
  advanceMaster(root, "b.txt", "master\n");
  stage(root, "a.txt", "staged\n");
  const before = git(root, "rev-parse", "HEAD");
  const outcome = syncMaster(root);
  assert.equal(outcome.ok, false);
  assert.equal(git(root, "rev-parse", "HEAD"), before);
  assert.equal(git(root, "show", ":a.txt"), "staged");
  assert.equal(git(root, "status", "--porcelain"), "M  a.txt");
});

test("sync refuses unstaged, untracked, master and in-progress states", () => {
  const unstaged = repository();
  advanceMaster(unstaged, "m.txt", "m\n");
  writeFileSync(join(unstaged, "a.txt"), "unstaged\n");
  assert.equal(syncMaster(unstaged).ok, false);
  assert.equal(git(unstaged, "diff", "--name-only"), "a.txt");

  const untracked = repository();
  advanceMaster(untracked, "m.txt", "m\n");
  writeFileSync(join(untracked, "new.txt"), "x\n");
  assert.equal(syncMaster(untracked).ok, false);

  const onMaster = repository();
  advanceMaster(onMaster, "m.txt", "m\n");
  git(onMaster, "switch", "-q", "master");
  assert.equal(syncMaster(onMaster).ok, false);

  const noRemote = repository();
  stage(noRemote, "a.txt", "staged\n");
  assert.equal(syncMaster(noRemote).ok, false);
  assert.equal(git(noRemote, "show", ":a.txt"), "staged");

  const merging = repository();
  advanceMaster(merging, "m.txt", "m\n");
  writeFileSync(
    join(gitDirectory(merging), "MERGE_HEAD"),
    `${git(merging, "rev-parse", "origin/master")}\n`,
  );
  assert.equal(syncMaster(merging).ok, false);
});

test("blocked record allows an uncommitted exit only for the recorded state", () => {
  const root = repository();
  writeFileSync(join(root, "a.txt"), "dirty\n");
  assert.equal(recordBlocked(root, "").ok, false);
  assert.equal(recordBlocked(root, "두 줄\n이유").ok, false);
  assert.equal(blockedExit(root), false);
  assert.equal(recordBlocked(root, "check 실패 원인을 못 찾음").ok, true);
  assert.equal(blockedExit(root), true);

  writeFileSync(join(root, "a.txt"), "more\n");
  assert.equal(blockedExit(root), false);
  assert.equal(recordBlocked(root, "다시 막힘").ok, true);
  writeFileSync(join(root, "c.txt"), "new\n");
  assert.equal(blockedExit(root), false);
  rmSync(join(root, "c.txt"));
  assert.equal(blockedExit(root), true);

  git(root, "commit", "-q", "-am", "moved");
  writeFileSync(join(root, "a.txt"), "after\n");
  assert.equal(blockedExit(root), false);

  const marker = join(gitDirectory(root), BLOCKED_FILE);
  writeFileSync(
    marker,
    JSON.stringify({ report: "완료", head: "x", digest: "y" }),
  );
  assert.equal(blockedExit(root), false);
  writeFileSync(marker, "not json");
  assert.equal(blockedExit(root), false);
});

const f = createFixture();
for (const path of [
  "harness/sandbox/policy.json",
  "harness/arch-check/rules.toml",
  "harness/claude-hooks/agent-rules.json",
]) {
  mkdirSync(join(f.root, path, ".."), { recursive: true });
  cpSync(join(repoRoot, path), join(f.root, path));
}
after(() => rmSync(f.base, { recursive: true, force: true }));

const stop = (facts) =>
  antigravityStop(
    {
      hookEventName: "Stop",
      cwd: f.worktree,
      modelName: "gemini-4-neon-xhigh",
      terminationReason: "NO_TOOL_CALL",
      fullyIdle: true,
    },
    { project: projectFor(f, { lock: LOCK.on }), root: f.worktree },
    { branch: "h-67", hooksReady: true, ...facts },
  );

test("implementer stop allows a dirty exit only after a recorded block", () => {
  assert.equal(
    stop({ dirty: " M crates/domain/src/lib.rs", blockedExit: true }).decision,
    "allow",
  );
  const refused = stop({
    dirty: " M crates/domain/src/lib.rs",
    blockedExit: false,
  });
  assert.equal(refused.decision, "continue");
  assert.match(refused.reason, /staged-sync\.mjs blocked/u);
  assert.equal(stop({ dirty: "", blockedExit: false }).decision, "allow");
});

const pre = (command, lock) =>
  antigravityPreTool(
    {
      hookEventName: "PreToolUse",
      cwd: f.worktree,
      modelName: "gemini-4-neon-xhigh",
      toolCall: {
        id: "tc-67",
        name: "run_command",
        args: { CommandLine: command },
      },
    },
    projectFor(f, { lock }),
    f.worktree,
    { home: join(f.base, "home"), branch: "h-67", threadId: undefined },
  );

test("the staged sync commands are the narrow path and raw bypasses stay denied", () => {
  for (const lock of [LOCK.on, LOCK.off]) {
    for (const command of [
      "node harness/antigravity-hooks/staged-sync.mjs sync",
      "node harness/antigravity-hooks/staged-sync.mjs blocked check 실패",
    ]) {
      assert.equal(pre(command, lock).decision, "allow", `${lock} ${command}`);
    }
    for (const command of [
      "git stash",
      "git stash push --staged",
      "git reset --merge",
      `echo x > .git/${BLOCKED_FILE}`,
      `printf x > $(git rev-parse --git-dir)/${BLOCKED_FILE}`,
    ]) {
      assert.equal(pre(command, lock).decision, "deny", `${lock} ${command}`);
    }
  }
});
