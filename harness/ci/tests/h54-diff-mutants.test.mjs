import assert from "node:assert/strict";
import { test } from "node:test";
import { spawnSync } from "node:child_process";
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
  );
  assert.deepEqual(calls, ["0/4", "1/4", "2/4", "3/4"]);
  assert.deepEqual(written, ["passed"]);
});
