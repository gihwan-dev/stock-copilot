import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { join } from "node:path";
import { repoRoot } from "../../claude-hooks/tests/fixture.mjs";
test("R3 dispatch exceptions emit a canonical PreToolUse denial with exit zero and stderr", () => {
  const run = spawnSync(
    process.execPath,
    [join(repoRoot, "harness/codex-hooks/dispatch.mjs")],
    { cwd: repoRoot, input: "{bad", encoding: "utf8" },
  );
  assert.equal(run.status, 0, run.stderr);
  assert.match(run.stderr, /하네스 오류/u);
  assert.equal(
    JSON.parse(run.stdout).hookSpecificOutput.permissionDecision,
    "deny",
  );
});
