import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";
import { test } from "node:test";
import { desktopRoot, repoRoot } from "./paths.mjs";

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
const desktopManifest = readJson(join(desktopRoot, "package.json"));
const rootManifest = readJson(join(repoRoot, "package.json"));
const bin = (name) => join(desktopRoot, "node_modules", ".bin", name);
const exactVersion = /^(npm:(@[a-z0-9-]+\/)?[a-z0-9-]+@)?\d+\.\d+\.\d+$/;

test("TypeScript 7.0.2 and the TypeScript 6 API are pinned with npm aliases (I-16)", () => {
  assert.equal(
    desktopManifest.devDependencies["@typescript/native"],
    "npm:typescript@7.0.2",
  );
  assert.equal(
    desktopManifest.devDependencies.typescript,
    "npm:@typescript/typescript6@6.0.2",
  );
});

test("tsc is TypeScript 7.0.2", () => {
  const result = spawnSync(bin("tsc"), ["--version"], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim(), "Version 7.0.2");
});

test("tsc6 is TypeScript 6.0", () => {
  const result = spawnSync(bin("tsc6"), ["--version"], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout.trim(), /^Version 6\.0\.\d+$/);
});

test("the typescript module seen by lint tools is the TypeScript 6.0 API", () => {
  const ts = createRequire(join(desktopRoot, "package.json"))("typescript");
  assert.match(ts.version, /^6\.0\.\d+$/);
  assert.equal(typeof ts.createProgram, "function");
});

test("Tauri CLI is pinned to the version measured in ADR 0004", () => {
  assert.equal(desktopManifest.devDependencies["@tauri-apps/cli"], "2.12.4");
  const result = spawnSync(bin("tauri"), ["--version"], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim(), "tauri-cli 2.12.4");
});

test("every npm dependency is pinned to an exact version", () => {
  const sections = [rootManifest, desktopManifest].flatMap((manifest) => [
    manifest.dependencies ?? {},
    manifest.devDependencies ?? {},
  ]);
  const loose = sections.flatMap((deps) =>
    Object.entries(deps).filter(([, version]) => !exactVersion.test(version)),
  );
  assert.deepEqual(loose, []);
});

test("pnpm is pinned to an exact version with a sha512 integrity hash", () => {
  const pinned = /^pnpm@(\d+\.\d+\.\d+)\+sha512\.[0-9a-f]{128}$/.exec(
    rootManifest.packageManager,
  );
  assert.notEqual(pinned, null, rootManifest.packageManager);
  const result = spawnSync("pnpm", ["--version"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim(), pinned[1]);
});
