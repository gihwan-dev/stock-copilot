import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { runShards, verifyShard } from "../diff-mutants.mjs";

const catalog = Array.from({ length: 25 }, (_, index) => ({
  name: `mutation-${index}`,
  file: `file-${index % 4}.rs`,
}));
const success = (length) => ({
  total_mutants: length,
  outcomes: [{ scenario: "Baseline", summary: "Success" }],
});

test("네 변경분 조각은 겹침 없이 전체 목록을 완주해야 한다", () => {
  const seen = [];
  for (let index = 0; index < 4; index += 1) {
    const shard = catalog.slice(index * 7, (index + 1) * 7);
    assert.doesNotThrow(() =>
      verifyShard(catalog, catalog, shard, success(shard.length), index),
    );
    seen.push(...shard);
  }
  assert.deepEqual(seen, catalog);
  assert.throws(() =>
    verifyShard(catalog, catalog, catalog.slice(1, 8), success(7), 0),
  );
  assert.throws(() =>
    verifyShard(catalog, catalog.slice(1), catalog.slice(0, 7), success(7), 0),
  );
  assert.throws(() =>
    verifyShard(catalog, catalog, catalog.slice(0, 7), success(6), 0),
  );
  assert.throws(() =>
    verifyShard(
      catalog,
      catalog,
      catalog.slice(0, 7),
      { total_mutants: 7, outcomes: [] },
      0,
    ),
  );
  assert.throws(() =>
    verifyShard(catalog, catalog, catalog.slice(0, 7), success(7), 4),
  );
  assert.throws(() => verifyShard(catalog, catalog, [], undefined, 4));
  assert.doesNotThrow(() => verifyShard([], [], [], undefined, 3));
});

test("변경분 변이의 최종 판정은 완주만 허용한다", () => {
  for (const [status, expected] of [
    ["passed", 0],
    ["deadline", 1],
    ["", 1],
    ["unknown", 1],
  ]) {
    const result = spawnSync(
      process.execPath,
      ["harness/ci/diff-mutants-verdict.mjs"],
      {
        env: { ...process.env, MUTATION_STATUS: status },
        encoding: "utf8",
      },
    );
    assert.equal(result.status, expected, status);
  }
});

test("조각 실패와 제한 초과는 성공 영수증을 만들지 않는다", () => {
  const base = "a".repeat(40);
  for (const status of [1, 124, null]) {
    const calls = [];
    const written = [];
    const run = (_, shard) => {
      calls.push(shard);
      return status;
    };
    const execute = () =>
      runShards(
        base,
        run,
        () => [],
        (value) => written.push(value),
        () => 0,
      );
    if (status === 124) {
      assert.doesNotThrow(execute);
      assert.deepEqual(written, ["deadline"]);
    } else {
      assert.throws(execute);
      assert.deepEqual(written, []);
    }
    assert.deepEqual(calls, ["0/4"]);
  }
  const calls = [];
  const written = [];
  runShards(
    base,
    (_, shard) => {
      calls.push(shard);
      return 0;
    },
    () => [],
    (value) => written.push(value),
    () => 0,
  );
  assert.deepEqual(calls, ["0/4", "1/4", "2/4", "3/4"]);
  assert.deepEqual(written, ["passed"]);
});

test("선택 수가 늘면 일곱 개 이하로 분할하고 목록을 모두 검사한다", () => {
  const selected = Array.from({ length: 100 }, (_, index) => ({
    name: `mutation-${index}`,
    file: "file.rs",
  }));
  let current;
  const calls = [];
  const written = [];
  runShards(
    "a".repeat(40),
    (_, shard) => {
      current = shard;
      calls.push(shard);
      return 0;
    },
    (path) => {
      if (path.endsWith("catalog.json")) return selected;
      const [index, count] = current.split("/").map(Number);
      const length = Math.ceil(selected.length / count);
      const slice = selected.slice(index * length, (index + 1) * length);
      return path.endsWith("mutants.json") ? slice : success(slice.length);
    },
    (value) => written.push(value),
    () => 0,
  );
  assert.equal(calls.length, 15);
  assert.equal(calls[0], "0/16");
  assert.equal(calls.at(-1), "14/16");
  assert.deepEqual(written, ["passed"]);
});

test("제한 초과한 조각은 더 잘게 나누어 전체 조각을 다시 완주해야 한다", () => {
  let current;
  const calls = [];
  const written = [];
  runShards(
    "a".repeat(40),
    (_, shard) => {
      current = shard;
      calls.push(shard);
      return shard === "1/4" ? 124 : 0;
    },
    (path) => {
      if (path.endsWith("catalog.json")) return catalog;
      const [index, count] = current.split("/").map(Number);
      const length = Math.ceil(catalog.length / count);
      const slice = catalog.slice(index * length, (index + 1) * length);
      return path.endsWith("mutants.json") ? slice : success(slice.length);
    },
    (value) => written.push(value),
    () => 0,
  );
  assert.deepEqual(calls, [
    "0/4",
    "1/4",
    ...Array.from({ length: 7 }, (_, index) => `${index}/8`),
  ]);
  assert.deepEqual(written, ["passed"]);
});

test("계획 실패와 조각 목록 변경은 성공으로 처리하지 않는다", () => {
  const written = [];
  let runs = 0;
  assert.throws(() =>
    runShards(
      "a".repeat(40),
      () => {
        runs += 1;
        return 0;
      },
      () => catalog,
      (value) => written.push(value),
      () => 1,
    ),
  );
  assert.equal(runs, 0);
  assert.deepEqual(written, []);
  let reads = 0;
  assert.throws(() =>
    runShards(
      "a".repeat(40),
      () => 0,
      () => {
        reads += 1;
        return reads === 1 ? catalog : catalog.slice(1);
      },
      (value) => written.push(value),
      () => 0,
    ),
  );
  assert.deepEqual(written, []);
});

test("더 나눌 수 없는 제한 초과는 재시도 판정으로 남는다", () => {
  const written = [];
  const calls = [];
  runShards(
    "a".repeat(40),
    (_, shard) => {
      calls.push(shard);
      return 124;
    },
    () => [catalog[0]],
    (value) => written.push(value),
    () => 0,
  );
  assert.deepEqual(calls, ["0/4"]);
  assert.deepEqual(written, ["deadline"]);
});

test("just는 변이 명령의 제한 초과 종료 코드를 보존한다", (t) => {
  const parent = resolve("target/H-54");
  mkdirSync(parent, { recursive: true });
  const directory = mkdtempSync(join(parent, "just-exit-"));
  t.after(() => rmSync(directory, { recursive: true, force: true }));
  const file = join(directory, "justfile");
  writeFileSync(file, "deadline:\n    exit 124\n");
  const result = spawnSync("just", ["--justfile", file, "deadline"], {
    encoding: "utf8",
  });
  assert.equal(result.status, 124, result.stderr);
});
