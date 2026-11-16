import { readFileSync } from "node:fs";
import { chromium } from "@playwright/test";
import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  baselineProblems,
  environmentProblems,
  compareCaptures,
  settings,
} from "../../../harness/visual/policy.mjs";

const cases = JSON.parse(
  readFileSync(
    new URL("../../../harness/visual/cases.json", import.meta.url),
    "utf8",
  ),
);

test("DS15 rejects darwin baselines including fixed violation paths", () => {
  assert.deepEqual(baselineProblems(cases.allowed), []);
  for (const path of cases.rejected)
    assert.equal(baselineProblems([path]).length, 1);
});

test("DS15 baseline writes require the exact container environment", () => {
  const valid = {
    platform: "linux",
    arch: "arm64",
    container: "present",
    image: settings.image,
  };
  assert.deepEqual(environmentProblems(valid), []);
  for (const [key, value] of Object.entries({
    platform: "darwin",
    arch: "x64",
    container: "absent",
    image: "playwright:latest",
  }))
    assert.equal(environmentProblems({ ...valid, [key]: value }).length, 1);
});

test("DS15 byte comparison rejects missing, empty, and changed captures", () => {
  assert.deepEqual(
    compareCaptures(Buffer.from([1, 2, 3]), Buffer.from([1, 2, 3])),
    [],
  );
  assert.equal(
    compareCaptures(Buffer.from([1, 2, 3]), Buffer.from([1, 3, 2])).length,
    1,
  );
  assert.equal(compareCaptures(Buffer.alloc(0), Buffer.alloc(0)).length, 1);
  assert.equal(compareCaptures(undefined, Buffer.from([1])).length, 1);
  assert.equal(compareCaptures(Buffer.from([1]), undefined).length, 1);
});

test("DS15 locks all rendering inputs independently", () => {
  assert.equal(
    settings.image,
    "mcr.microsoft.com/playwright:v1.65.0-noble@sha256:9beb546099e11f52d62e505537914870e01a67e0e6b00ae41da4775324e73dbe",
  );
  assert.equal(settings.engine, "chromium");
  assert.equal(chromium.name(), settings.engine);
  assert.deepEqual(settings.viewport, { width: 960, height: 640 });
  assert.equal(settings.scale, 1);
  assert.equal(settings.locale, "ko-KR");
  assert.equal(settings.timezone, "Asia/Seoul");
  assert.equal(settings.time, "2026-01-02T03:04:05.000Z");
  assert.equal(settings.random, 0.3141592653589793);
  assert.equal(settings.animations, "disabled");
  assert.equal(settings.caret, "hide");
  assert.deepEqual(settings.components, [
    "Button",
    "Card",
    "Money",
    "TextField",
  ]);
  assert.deepEqual(settings.themes, ["light", "dark"]);
});

test("DS15 real entry refuses an untrusted image before any baseline write", async () => {
  const { spawnSync } = await import("node:child_process");
  const { fileURLToPath } = await import("node:url");
  const run = spawnSync(
    process.execPath,
    [
      fileURLToPath(
        new URL("../../../harness/visual/run.mjs", import.meta.url),
      ),
      "update",
    ],
    {
      encoding: "utf8",
      env: { ...process.env, VISUAL_IMAGE: "wrong" },
    },
  );
  assert.equal(run.status, 1);
  assert.match(run.stderr, /기준 환경 불일치: image/);
});

test("DS15 actual registered checker rejects fixed darwin files in another repository", async () => {
  const { spawnSync } = await import("node:child_process");
  const { mkdtempSync, mkdirSync, writeFileSync, rmSync } =
    await import("node:fs");
  const { join, dirname } = await import("node:path");
  const { fileURLToPath } = await import("node:url");
  const root = fileURLToPath(new URL("../../../", import.meta.url));
  const folder = mkdtempSync(join(root, "target/DS-15-cli-"));
  const checker = fileURLToPath(
    new URL("../../../harness/visual/check.mjs", import.meta.url),
  );
  try {
    assert.equal(spawnSync("git", ["init", "--quiet", folder]).status, 0);
    const check = () =>
      spawnSync(process.execPath, [checker, folder], { encoding: "utf8" });
    assert.equal(check().status, 0);
    for (const path of cases.rejected) {
      mkdirSync(dirname(join(folder, path)), { recursive: true });
      writeFileSync(join(folder, path), Buffer.from([137, 80, 78, 71]));
      const run = check();
      assert.equal(run.status, 1);
      assert.ok(run.stderr.includes(path));
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});

test("DS15 browser startup failure closes its server and exits", async () => {
  const { spawnSync } = await import("node:child_process");
  const { mkdtempSync, mkdirSync, writeFileSync, rmSync } =
    await import("node:fs");
  const { join, dirname } = await import("node:path");
  const { fileURLToPath } = await import("node:url");
  const root = fileURLToPath(new URL("../../../", import.meta.url));
  const folder = mkdtempSync(join(root, "target/DS-15-startup-"));
  const put = (path, content) => {
    mkdirSync(dirname(join(folder, path)), { recursive: true });
    writeFileSync(join(folder, path), content);
  };
  try {
    put(
      "harness/visual/run.mjs",
      readFileSync(new URL("../../../harness/visual/run.mjs", import.meta.url)),
    );
    put(
      "harness/visual/policy.mjs",
      `export const settings=${JSON.stringify(settings)}; export const environmentProblems=()=>[]; export const compareCaptures=()=>[];`,
    );
    for (const module of ["capture", "server"])
      put(
        `harness/visual/${module}.mjs`,
        readFileSync(
          new URL(`../../../harness/visual/${module}.mjs`, import.meta.url),
        ),
      );
    put("apps/desktop/package.json", "{}");
    put(
      "apps/desktop/node_modules/@playwright/test/package.json",
      '{"main":"index.cjs"}',
    );
    put(
      "apps/desktop/node_modules/@playwright/test/index.cjs",
      'exports.chromium={launch:async()=>{throw new Error("forced launch failure")}};',
    );
    put("target/DS-15/storybook/index.json", '{"entries":{}}');
    const run = spawnSync(
      process.execPath,
      [join(folder, "harness/visual/run.mjs"), "measure"],
      { encoding: "utf8", timeout: 10000 },
    );
    assert.equal(run.error, undefined);
    assert.equal(run.status, 1);
    assert.match(run.stderr, /forced launch failure/);
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
