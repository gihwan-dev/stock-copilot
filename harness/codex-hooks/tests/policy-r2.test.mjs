import { strict as assert } from "node:assert";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import * as config from "../config.mjs";
import * as prelock from "../../sandbox/prelock.mjs";
import { createFixture, repoRoot } from "../../claude-hooks/tests/fixture.mjs";
const policy = JSON.parse(
  readFileSync(join(repoRoot, "harness/sandbox/policy.json"), "utf8"),
);
test("R2 every filesystem and socket policy entry has explicit coverage including non home entries", () => {
  assert.equal(typeof config.policyCoverage, "function");
  const rows = config.policyCoverage(policy);
  for (const [kind, paths] of Object.entries(policy.base.filesystem))
    for (const path of paths) {
      const found = rows.filter(
        (row) => row.kind === kind && row.path === path,
      );
      assert.equal(found.length, 1, kind + " " + path);
      assert.ok(found[0].target && found[0].reason);
      const removed = structuredClone(policy);
      removed.base.filesystem[kind] = paths.filter((value) => value !== path);
      assert.notDeepEqual(config.policyCoverage(removed), rows, path);
    }
  for (const path of policy.base.network.allowUnixSockets)
    assert.ok(
      rows.some((row) => row.kind === "allowUnixSockets" && row.path === path),
    );
  for (const [kind, path] of [
    ["allowWrite", "/unmapped/cache"],
    ["allowWrite", "../new-worktrees"],
    ["denyWrite", "/unmapped/config"],
  ]) {
    const changed = structuredClone(policy);
    changed.base.filesystem[kind].push(path);
    assert.throws(() => config.policyCoverage(changed), /정책/u);
  }
});
test("R2 session Git writes exclude HEAD and the administrative directory grant", () => {
  const f = createFixture();
  try {
    const text = config.setupText(f.worktree);
    const parent = join(f.root, ".git/worktrees/worktree");
    assert.ok(text.includes(JSON.stringify(parent + "/HEAD") + ' = "read"'));
    assert.ok(!text.includes(JSON.stringify(parent) + ' = "write"'));
    for (const name of [
      "index",
      "index.lock",
      "ORIG_HEAD",
      "MERGE_HEAD",
      "MERGE_MSG",
      "COMMIT_EDITMSG",
      "logs/HEAD",
    ])
      assert.ok(
        text.includes(JSON.stringify(parent + "/" + name) + ' = "write"'),
        name,
      );
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
test("R2 prelock detects a pasted harness profile without implicitly reading user files", () => {
  assert.equal(typeof prelock.codexUserProfileProblems, "function");
  assert.deepEqual(
    prelock.codexUserProfileProblems(
      '[permissions.stock-copilot-session]\nextends="stock-copilot"\n',
    ),
    [],
  );
  for (const text of [
    "[permissions.stock-copilot-harness]\n",
    '[permissions."stock-copilot-harness".filesystem]\n',
    'default_permissions="stock-copilot-harness"\n',
  ])
    assert.ok(prelock.codexUserProfileProblems(text).length);
});

test("R2 prelock user config scanning is explicit and blocks pasted profiles", () => {
  const f = createFixture();
  try {
    const path = join(f.base, "config.toml");
    writeFileSync(path, "[permissions.stock-copilot-harness]\\n");
    const run = (args) =>
      spawnSync(
        process.execPath,
        ["harness/sandbox/prelock.mjs", "HEAD", ...args],
        {
          cwd: repoRoot,
          env: { ...process.env, CODEX_HOME: f.base },
          encoding: "utf8",
        },
      );
    assert.doesNotMatch(run([]).stdout, /사용자 설정에 하네스 프로필/u);
    const checked = run(["--codex-user-config", path]);
    assert.equal(checked.status, 1);
    assert.match(checked.stdout, /사용자 설정에 하네스 프로필/u);
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
