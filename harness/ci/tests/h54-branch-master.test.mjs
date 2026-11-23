import { strict as assert } from "node:assert";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { mkdtempSync } from "../test-temp.mjs";
import { repoRoot } from "./signature-repo.mjs";

const environment = () => ({
  ...Object.fromEntries(
    Object.entries(process.env).filter(([key]) => key !== "NODE_TEST_CONTEXT"),
  ),
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_TERMINAL_PROMPT: "0",
});

const run = (cwd, command, args) =>
  spawnSync(command, args, { cwd, env: environment(), encoding: "utf8" });

test("로컬 master가 없는 브랜치 복제본에서 훅 진입 테스트가 통과한다", () => {
  const parent = mkdtempSync(join(tmpdir(), "h54-branch-"));
  const clone = join(parent, "repo");
  const made = run(repoRoot, "git", [
    "clone",
    "--quiet",
    "--shared",
    ".",
    clone,
  ]);
  assert.equal(made.status, 0, made.stderr);
  const head = run(clone, "git", ["rev-parse", "HEAD"]);
  assert.equal(head.status, 0, head.stderr);
  const checkout = run(clone, "git", [
    "checkout",
    "--quiet",
    "-B",
    "h54-fixture",
    head.stdout.trim(),
  ]);
  assert.equal(checkout.status, 0, checkout.stderr);
  const local = run(clone, "git", [
    "show-ref",
    "--verify",
    "--quiet",
    "refs/heads/master",
  ]);
  if (local.status === 0) {
    const removed = run(clone, "git", ["branch", "-D", "master"]);
    assert.equal(removed.status, 0, removed.stderr);
  }
  const fetched = run(clone, "git", [
    "fetch",
    "--quiet",
    "--no-tags",
    repoRoot,
    "+refs/remotes/origin/master:refs/remotes/origin/master",
  ]);
  assert.equal(fetched.status, 0, fetched.stderr);
  assert.notEqual(
    run(clone, "git", ["show-ref", "--verify", "--quiet", "refs/heads/master"])
      .status,
    0,
  );
  const source = run(repoRoot, "git", [
    "rev-parse",
    "refs/remotes/origin/master^{commit}",
  ]);
  const tracked = run(clone, "git", [
    "rev-parse",
    "refs/remotes/origin/master^{commit}",
  ]);
  assert.equal(source.status, 0, source.stderr);
  assert.equal(tracked.status, 0, tracked.stderr);
  assert.equal(tracked.stdout, source.stdout);
  const branchCheck = "harness/ci/branch-checkout.mjs";
  cpSync(join(repoRoot, branchCheck), join(clone, branchCheck));
  assert.equal(run(clone, process.execPath, [branchCheck]).status, 0);
  const extraMaster = run(clone, "git", [
    "update-ref",
    "refs/heads/master",
    tracked.stdout.trim(),
  ]);
  assert.equal(extraMaster.status, 0, extraMaster.stderr);
  assert.equal(run(clone, process.execPath, [branchCheck]).status, 1);
  const removeMaster = run(clone, "git", [
    "update-ref",
    "-d",
    "refs/heads/master",
  ]);
  assert.equal(removeMaster.status, 0, removeMaster.stderr);
  for (const file of [
    "harness/ci/managed-temp.mjs",
    "harness/ci/test-temp.mjs",
  ]) {
    rmSync(join(clone, file), { force: true });
    cpSync(join(repoRoot, file), join(clone, file));
  }
  const testFile = "harness/claude-hooks/tests/agent-stop.test.mjs";
  mkdirSync(join(clone, "harness/claude-hooks/tests"), { recursive: true });
  cpSync(join(repoRoot, testFile), join(clone, testFile));
  const result = run(clone, process.execPath, [
    "--test",
    "--test-name-pattern=hook entry scripts record a start",
    testFile,
  ]);
  assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  assert.match(result.stdout, /pass 1/u);
  const deleted = run(clone, "git", [
    "update-ref",
    "-d",
    "refs/remotes/origin/master",
  ]);
  assert.equal(deleted.status, 0, deleted.stderr);
  assert.equal(run(clone, process.execPath, [branchCheck]).status, 1);
  const missing = run(clone, process.execPath, [
    "--test",
    "--test-name-pattern=hook entry scripts record a start",
    testFile,
  ]);
  assert.equal(missing.status, 1);
  assert.match(
    missing.stdout + missing.stderr,
    /refs\/remotes\/origin\/master/u,
  );
});
