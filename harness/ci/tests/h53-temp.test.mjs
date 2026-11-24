import assert from "node:assert/strict";
import { test } from "node:test";
import { spawn, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { parseSessionGroupsOutput, runTempAudit } from "../temp-audit.mjs";

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

test("임시 검사기는 남아 있는 자손을 실패로 보고 정리한다", (t) => {
  const directory = fixture(t);
  const source = `const child = require("child_process").spawn(process.execPath, ["-e", "setInterval(() => {}, 1000)"], {stdio: "ignore"}); child.unref();`;
  const result = spawnSync(
    process.execPath,
    [audit, process.execPath, "-e", source],
    {
      env: { ...environment(), TMPDIR: directory },
      encoding: "utf8",
      timeout: 10000,
    },
  );
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /자손 프로세스/u);
  assert.deepEqual(readdirSync(directory), []);
});

test("임시 검사기는 같은 세션에서 새 그룹을 만든 자손도 정리한다", async (t) => {
  const directory = fixture(t);
  const pidFile = join(directory, "owned-child.pid");
  const source =
    'set -m; sleep 30 </dev/null >/dev/null 2>&1 & echo $! > "$PIDFILE"';
  const result = spawnSync(process.execPath, [audit, "/bin/sh", "-c", source], {
    env: { ...environment(), TMPDIR: directory, PIDFILE: pidFile },
    encoding: "utf8",
    timeout: 10000,
  });
  const pid = Number(readFileSync(pidFile, "utf8"));
  let alive = true;
  for (let attempt = 0; attempt < 40 && alive; attempt += 1) {
    try {
      process.kill(pid, 0);
      await new Promise((done) => setTimeout(done, 25));
    } catch (error) {
      if (error.code !== "ESRCH") throw error;
      alive = false;
    }
  }
  if (alive) process.kill(pid, "SIGTERM");
  assert.equal(alive, false, "owned child remains alive");
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /자손 프로세스/u);
  assert.deepEqual(readdirSync(directory), ["owned-child.pid"]);
});

test("소유 세션 조회기를 실행하지 못하면 감사는 실패한다", (t) => {
  const directory = fixture(t);
  const result = spawnSync(
    process.execPath,
    [audit, "/bin/sh", "-c", "exit 0"],
    {
      env: { ...environment(), TMPDIR: directory, PATH: directory },
      encoding: "utf8",
      timeout: 10000,
    },
  );
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /소유 세션을 확인하지 못했다/u);
  assert.deepEqual(readdirSync(directory), []);
});

test("소유 세션 조회 결과가 불완전하면 감사는 실패한다", (t) => {
  const directory = fixture(t);
  const systemTrue = ["/usr/bin/true", "/bin/true"].find((candidate) =>
    existsSync(candidate),
  );
  assert.ok(systemTrue !== undefined);
  symlinkSync(systemTrue, join(directory, "python3"));
  const result = spawnSync(
    process.execPath,
    [audit, "/bin/sh", "-c", "exit 0"],
    {
      env: { ...environment(), TMPDIR: directory, PATH: directory },
      encoding: "utf8",
      timeout: 10000,
    },
  );
  assert.equal(result.status, 1, result.stderr);
  assert.match(result.stderr, /소유 세션을 확인하지 못했다/u);
  assert.deepEqual(readdirSync(directory), ["python3"]);
});

test("세션 조회 출력 파서는 불완전하거나 잘못된 응답을 모두 거부한다", () => {
  for (const invalid of [
    { status: 0, stdout: "" },
    { status: 1, stdout: "ok\n" },
    { status: 0, stdout: "ok\n", error: new Error("enoent") },
    { status: 0, stdout: "123\n" },
    { status: 0, stdout: "ok\n\n123\n" },
    { status: 0, stdout: "ok\nabc\n" },
    { status: 0, stdout: "ok\n+123\n" },
    { status: 0, stdout: "ok\n 123\n" },
    { status: 0, stdout: "ok\n0x10\n" },
    { status: 0, stdout: "ok\n1e2\n" },
    { status: 0, stdout: "ok\n0\n" },
    { status: 0, stdout: "ok\n-5\n" },
    { status: 0, stdout: "ok\n9007199254740993\n" },
  ]) {
    assert.equal(parseSessionGroupsOutput(invalid), undefined);
  }
  assert.deepEqual(parseSessionGroupsOutput({ status: 0, stdout: "ok\n" }), []);
  assert.deepEqual(
    parseSessionGroupsOutput({ status: 0, stdout: "ok\n42\n42\n99\n" }),
    [42, 99],
  );
});

test("주입한 세션 조회 가짜가 불완전한 결과를 주면 감사는 부하와 무관하게 즉시 실패한다", async (t) => {
  const directory = fixture(t);
  let out = "";
  let err = "";
  let calledWith;
  const outcome = await runTempAudit("/bin/sh", ["-c", "exit 0"], {
    baseTempDir: directory,
    env: environment(),
    stdio: "ignore",
    stdout: {
      write: (chunk) => {
        out += chunk;
      },
    },
    stderr: {
      write: (chunk) => {
        err += chunk;
      },
    },
    inspectSessionGroups: (pid) => {
      calledWith = pid;
      return undefined;
    },
  });
  assert.equal(typeof calledWith, "number");
  assert.equal(outcome.exitCode, 1);
  assert.match(err, /소유 세션을 확인하지 못했다/u);
  assert.match(out, /temp-audit: before=0 after=0/u);
  assert.deepEqual(readdirSync(directory), []);
});

test("주입한 세션 조회 가짜가 잔존 그룹을 보고하면 종료 신호를 보내고 실패한다", async (t) => {
  const directory = fixture(t);
  let err = "";
  const signals = [];
  let calls = 0;
  const outcome = await runTempAudit("/bin/sh", ["-c", "exit 0"], {
    baseTempDir: directory,
    env: environment(),
    stdio: "ignore",
    stdout: { write: () => {} },
    stderr: {
      write: (chunk) => {
        err += chunk;
      },
    },
    wait: async () => {},
    signalGroup: (group, signal) => {
      signals.push([group, signal]);
    },
    inspectSessionGroups: () => {
      calls += 1;
      return calls <= 24 ? [777] : [];
    },
  });
  assert.equal(outcome.exitCode, 1);
  assert.match(err, /자손 프로세스/u);
  assert.deepEqual(signals, [
    [777, "SIGTERM"],
    [777, "SIGKILL"],
  ]);
  assert.deepEqual(readdirSync(directory), []);
});

test("임시 검사기는 Node의 컴파일 캐시를 정리하면서 누출은 계속 거부한다", (t) => {
  const directory = fixture(t);
  for (const [leak, expected] of [
    [false, 0],
    [true, 1],
  ]) {
    const source = `require("module").enableCompileCache(); ${leak ? 'require("fs").mkdirSync(require("path").join(process.env.TMPDIR, "h47-summary-left"));' : ""}`;
    const result = spawnSync(
      process.execPath,
      [audit, process.execPath, "-e", source],
      {
        env: { ...environment(), TMPDIR: directory },
        encoding: "utf8",
      },
    );
    assert.equal(result.status, expected, result.stderr);
    if (leak) assert.match(result.stderr, /h47-summary-left/u);
    assert.deepEqual(readdirSync(directory), []);
  }
});

test("임시 검사기는 실행 공간의 pnpm 저장소를 정리하면서 누출은 계속 거부한다", (t) => {
  const directory = fixture(t);
  for (const [leak, expected] of [
    [false, 0],
    [true, 1],
  ]) {
    const source = `const fs = require("fs"), path = require("path"); fs.mkdirSync(path.join(process.env.TMPDIR, ".pnpm-store")); ${leak ? 'fs.mkdirSync(path.join(process.env.TMPDIR, "h47-summary-left"));' : ""}`;
    const result = spawnSync(
      process.execPath,
      [audit, process.execPath, "-e", source],
      {
        env: { ...environment(), TMPDIR: directory },
        encoding: "utf8",
      },
    );
    assert.equal(result.status, expected, result.stderr);
    if (leak) assert.match(result.stderr, /h47-summary-left/u);
    assert.deepEqual(readdirSync(directory), []);
  }
});

test("임시 검사 공간에서도 Unix 소켓을 만들 수 있다", () => {
  const source =
    'const fs = require("fs"), net = require("net"), path = require("path"); const file = path.join(process.env.TMPDIR, `stock-copilot-net-unix-${process.pid}`); const server = net.createServer(); server.listen(file, () => server.close(() => { if (fs.existsSync(file)) fs.unlinkSync(file); }));';
  const result = spawnSync(
    process.execPath,
    [audit, process.execPath, "-e", source],
    {
      env: environment(),
      encoding: "utf8",
    },
  );
  assert.equal(result.status, 0, result.stderr);
});
