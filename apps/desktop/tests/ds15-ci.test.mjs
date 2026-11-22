import { strict as assert } from "node:assert";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { writeFileSync, readFileSync, rmSync, mkdirSync } from "node:fs";
import { mkdtempSync } from "../../../harness/ci/test-temp.mjs";
import { join } from "node:path";
import { repoRoot } from "./paths.mjs";

for (const [name, install, compare, expected] of [
  ["success", 0, 0, 0],
  ["install failure", 7, 0, 7],
  ["comparison failure", 0, 9, 9],
]) {
  test(`DS15 CI entry preserves frozen install ordering and ${name}`, () => {
    mkdirSync(join(repoRoot, "target"), { recursive: true });
    const folder = mkdtempSync(join(repoRoot, "target/DS-15-ci-entry-"));
    const trace = join(folder, "calls.jsonl");
    const fake = (command, exit) => {
      const script = `#!${process.execPath}\nimport {appendFileSync} from 'node:fs';appendFileSync(process.env.DS15_TRACE,JSON.stringify({command:${JSON.stringify(command)},args:process.argv.slice(2),cwd:process.cwd()})+'\\n');process.exitCode=${exit};\n`;
      writeFileSync(join(folder, command), script, { mode: 0o755 });
    };
    fake("pnpm", install);
    fake("just", compare);
    try {
      const run = spawnSync(
        process.execPath,
        [join(repoRoot, "harness/ci/visual.mjs")],
        {
          encoding: "utf8",
          cwd: folder,
          env: { ...process.env, PATH: folder, DS15_TRACE: trace },
        },
      );
      assert.equal(run.status, expected, run.stderr);
      const calls = readFileSync(trace, "utf8")
        .trim()
        .split("\n")
        .map((line) => JSON.parse(line));
      const wanted = [
        {
          command: "pnpm",
          args: ["install", "--frozen-lockfile", "--ignore-scripts"],
          cwd: repoRoot,
        },
      ];
      if (install === 0)
        wanted.push({ command: "just", args: ["visual-check"], cwd: repoRoot });
      assert.deepEqual(calls, wanted);
    } finally {
      rmSync(folder, { recursive: true, force: true });
    }
  });
}
