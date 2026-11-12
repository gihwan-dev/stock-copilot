import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { repoRoot } from "./paths.mjs";

const pinnedNode = "24.22.0";

const withFakeNode = (version, run) => {
  const dir = mkdtempSync(join(tmpdir(), "fake-node-"));
  const fake = join(dir, "node");
  writeFileSync(fake, `#!/bin/sh\nprintf '%s\\n' 'v${version}'\n`);
  chmodSync(fake, 0o755);
  const result = run({ ...process.env, PATH: `${dir}:${process.env.PATH}` });
  rmSync(dir, { recursive: true, force: true });
  return result;
};

test(".node-version pins the Node version from harness 1.3", () => {
  assert.equal(
    readFileSync(join(repoRoot, ".node-version"), "utf8"),
    `${pinnedNode}\n`,
  );
});

test("root package.json requires exactly the pinned Node version", () => {
  const manifest = JSON.parse(
    readFileSync(join(repoRoot, "package.json"), "utf8"),
  );
  assert.equal(manifest.engines.node, pinnedNode);
});

test("the running Node is the pinned version", () => {
  assert.equal(process.version, `v${pinnedNode}`);
});

test("just check fails at the Node pin when another Node version is on PATH", () => {
  const result = withFakeNode("24.21.0", (env) =>
    spawnSync("just", ["check"], { cwd: repoRoot, env, encoding: "utf8" }),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /node v24\.21\.0 .*v24\.22\.0/);
});

test("just check fails at the Node pin for a newer major version too", () => {
  const result = withFakeNode("26.0.0", (env) =>
    spawnSync("just", ["check"], { cwd: repoRoot, env, encoding: "utf8" }),
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /node v26\.0\.0 .*v24\.22\.0/);
});

test("the Node pin step passes with the pinned version", () => {
  const result = withFakeNode(pinnedNode, (env) =>
    spawnSync("just", ["check-node"], { cwd: repoRoot, env, encoding: "utf8" }),
  );
  assert.equal(result.status, 0, result.stderr);
});
