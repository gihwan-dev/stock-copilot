import assert from "node:assert/strict";
import { test } from "node:test";
import { spawn, spawnSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const root = resolve("target/H-53");
const environment = () =>
  Object.fromEntries(
    Object.entries(process.env).filter(([key]) => key !== "NODE_TEST_CONTEXT"),
  );
const audit = resolve("harness/ci/temp-audit.mjs");
const helper = pathToFileURL(resolve("harness/ci/test-temp.mjs")).href;

const fixture = (t) => {
  mkdirSync(root, { recursive: true });
  const directory = mkdtempSync(join(root, "temp-test-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  return directory;
};

test("임시 검사기는 실행별 공간에서 남은 항목을 실패로 보고 정리한다", (t) => {
  const directory = fixture(t);
  for (const [source, expected] of [
    ["process.exitCode = 0;", 0],
    ["process.exitCode = 7;", 7],
    [
      'require("fs").mkdirSync(require("path").join(process.env.TMPDIR, "h47-summary-left"));',
      1,
    ],
  ]) {
    const result = spawnSync(
      process.execPath,
      [audit, process.execPath, "-e", source],
      {
        env: { ...environment(), TMPDIR: directory },
        encoding: "utf8",
      },
    );
    assert.equal(result.status, expected, result.stderr);
    assert.deepEqual(readdirSync(directory), []);
    if (source.includes("h47-summary"))
      assert.match(result.stderr, /h47-summary-left/u);
  }
});

test("테스트 정리 훅은 실패한 테스트의 임시 폴더도 지운다", (t) => {
  const directory = fixture(t);
  const script = join(directory, "case.mjs");
  writeFileSync(
    script,
    `import {test} from 'node:test';\nimport {mkdtempSync} from ${JSON.stringify(helper)};\nimport {join} from 'node:path';\ntest('failure', () => { mkdtempSync(join(process.env.TMPDIR, 'h47-day-')); throw Error('expected'); });\n`,
  );
  const result = spawnSync(process.execPath, ["--test", script], {
    env: { ...environment(), TMPDIR: directory },
    encoding: "utf8",
  });
  assert.equal(result.status, 1);
  assert.deepEqual(readdirSync(directory), ["case.mjs"]);
});

test("임시 검사기는 중단된 자식의 공간도 지운다", async (t) => {
  const directory = fixture(t);
  const child = spawn(
    process.execPath,
    [
      audit,
      process.execPath,
      "-e",
      'require("fs").writeFileSync(process.env.READY, "ready"); setInterval(() => {}, 1000);',
    ],
    {
      env: {
        ...environment(),
        TMPDIR: directory,
        READY: join(directory, "ready"),
      },
      stdio: "ignore",
    },
  );
  t.after(() => {
    if (child.exitCode === null && child.signalCode === null)
      child.kill("SIGTERM");
  });
  const deadline = Date.now() + 5000;
  while (!readdirSync(directory).includes("ready") && Date.now() < deadline)
    await new Promise((done) => setTimeout(done, 20));
  assert.ok(readdirSync(directory).includes("ready"));
  child.kill("SIGTERM");
  const outcome = await new Promise((done) =>
    child.once("exit", (code, signal) => done({ code, signal })),
  );
  assert.ok(outcome.signal === "SIGTERM" || outcome.code === 143);
  assert.deepEqual(readdirSync(directory), ["ready"]);
});

test("검사 도우미는 작업이 예외를 내도 즉시 임시 폴더를 지운다", (t) => {
  const directory = fixture(t);
  for (const [module, call] of [
    [
      "harness/vitest-check/vitest.mjs",
      "withConfig({}, () => { throw Error('failure') })",
    ],
    [
      "harness/knip-check/knip.mjs",
      "withConfig({}, () => { throw Error('failure') })",
    ],
    [
      "harness/source-tree/tree.mjs",
      "withFixture('clean', () => { throw Error('failure') })",
    ],
  ]) {
    const url = pathToFileURL(resolve(module)).href;
    const source = `import * as check from ${JSON.stringify(url)}; import {readdirSync} from 'node:fs'; let thrown = false; try { check.${call}; } catch { thrown = true; } if (!thrown || readdirSync(process.env.TMPDIR).length !== 0) process.exitCode = 1;`;
    const result = spawnSync(
      process.execPath,
      ["--input-type=module", "-e", source],
      {
        env: { ...environment(), TMPDIR: directory },
        encoding: "utf8",
      },
    );
    assert.equal(result.status, 0, `${module}: ${result.stderr}`);
    for (const name of readdirSync(directory))
      rmSync(join(directory, name), { recursive: true, force: true });
  }
});

test("테스트 임시 폴더 관리자는 종료 신호에서도 자신이 만든 폴더를 지운다", async (t) => {
  const directory = fixture(t);
  const child = spawn(
    process.execPath,
    [
      "--input-type=module",
      "-e",
      `import {mkdtempSync} from ${JSON.stringify(helper)}; import {join} from 'node:path'; import {writeFileSync} from 'node:fs'; mkdtempSync(join(process.env.TMPDIR, 'agent-guard-')); writeFileSync(process.env.READY, 'ready'); setInterval(() => {}, 1000);`,
    ],
    {
      env: {
        ...environment(),
        TMPDIR: directory,
        READY: join(directory, "ready"),
      },
      stdio: "ignore",
    },
  );
  t.after(() => {
    if (child.exitCode === null && child.signalCode === null)
      child.kill("SIGTERM");
  });
  const deadline = Date.now() + 5000;
  while (!readdirSync(directory).includes("ready") && Date.now() < deadline)
    await new Promise((done) => setTimeout(done, 20));
  assert.ok(
    readdirSync(directory).some((name) => name.startsWith("agent-guard-")),
  );
  child.kill("SIGTERM");
  const outcome = await new Promise((done) =>
    child.once("exit", (code, signal) => done({ code, signal })),
  );
  assert.equal(outcome.signal, "SIGTERM");
  assert.deepEqual(readdirSync(directory), ["ready"]);
});
