import { strict as assert } from "node:assert";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

test("nightly mutation results preserve missed and timeout evidence in logs and job summary", () => {
  const root = mkdtempSync(join(tmpdir(), "h46-summary-"));
  const directory = join(root, "target/mutants/mutants.out");
  mkdirSync(directory, { recursive: true });
  writeFileSync(join(directory, "missed.txt"), "src/a.rs: missed witness\n");
  writeFileSync(join(directory, "timeout.txt"), "src/b.rs: timeout witness\n");
  const summary = join(root, "summary.md");
  const result = spawnSync(
    process.execPath,
    [resolve("harness/ci/mutants-summary.mjs")],
    {
      cwd: root,
      env: { ...process.env, GITHUB_STEP_SUMMARY: summary },
      encoding: "utf8",
    },
  );
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(summary, "utf8"), result.stdout);
  assert.match(result.stdout, /missed witness/u);
  assert.match(result.stdout, /timeout witness/u);
  assert.match(result.stdout, /결과 파일 없음/u);
});
