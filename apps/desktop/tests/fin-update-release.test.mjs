import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { pathToFileURL } from "node:url";
import { repoRoot } from "./paths.mjs";

const script = join(repoRoot, "scripts", "release-mac.mjs");
const release = await import(pathToFileURL(script).href);

const runScript = (args) =>
  spawnSync(process.execPath, [script, ...args], {
    cwd: repoRoot,
    encoding: "utf8",
    env: { PATH: "/nonexistent" },
  });

const watched = [
  "Cargo.toml",
  "Cargo.lock",
  "apps/desktop/src-tauri/tauri.conf.json",
];
const snapshot = () =>
  watched.map((path) => readFileSync(join(repoRoot, path), "utf8"));

test("fin-update release: only plain x.y.z versions are accepted", () => {
  assert.equal(release.validateVersion("0.2.0"), "0.2.0");
  assert.equal(release.validateVersion("10.20.30"), "10.20.30");
  for (const bad of ["1.2", "v1.2.3", "1.2.3-beta", "", undefined, "1.2.3 "]) {
    assert.throws(() => release.validateVersion(bad), release.ReleaseError);
  }
});

test("fin-update release: the workspace package version is the only version that changes", () => {
  const cargo = [
    "[workspace]",
    'members = ["a"]',
    "",
    "[workspace.package]",
    'version = "0.1.0"',
    'edition = "2024"',
    "",
    "[workspace.dependencies]",
    'tauri = { version = "=2.12.5" }',
    "",
    "[package]",
    'version = "9.9.9"',
    "",
  ].join("\n");
  const bumped = release.bumpCargoToml(cargo, "0.2.0");
  assert.equal(release.currentVersion(bumped), "0.2.0");
  assert.ok(bumped.includes('tauri = { version = "=2.12.5" }'));
  assert.ok(bumped.includes('[package]\nversion = "9.9.9"'));
  assert.equal(bumped.replace('version = "0.2.0"', 'version = "0.1.0"'), cargo);
  assert.throws(
    () => release.bumpCargoToml('[package]\nversion = "1.0.0"\n', "0.2.0"),
    release.ReleaseError,
  );
  const real = readFileSync(join(repoRoot, "Cargo.toml"), "utf8");
  assert.match(release.currentVersion(real), /^\d+\.\d+\.\d+$/u);
});

test("fin-update release: init-key writes the public key only over the placeholder", () => {
  const conf = readFileSync(
    join(repoRoot, "apps/desktop/src-tauri/tauri.conf.json"),
    "utf8",
  );
  const current = JSON.parse(conf).plugins.updater.pubkey;
  const sample = JSON.stringify(
    {
      plugins: {
        updater: {
          pubkey: release.PUBKEY_PLACEHOLDER,
          endpoints: [release.ENDPOINT],
        },
      },
    },
    null,
    2,
  );
  const written = release.writePubkey(sample, "dW50cnVzdGVkIGNvbW1lbnQ=\n");
  assert.equal(
    JSON.parse(written).plugins.updater.pubkey,
    "dW50cnVzdGVkIGNvbW1lbnQ=",
  );
  assert.deepEqual(JSON.parse(written).plugins.updater.endpoints, [
    release.ENDPOINT,
  ]);
  assert.throws(
    () => release.writePubkey(written, "b3RoZXI="),
    release.ReleaseError,
  );
  assert.throws(
    () => release.writePubkey(sample, "has space"),
    release.ReleaseError,
  );
  assert.throws(() => release.writePubkey("{}", "a2V5"), release.ReleaseError);
  assert.equal(typeof current, "string");
});

test("fin-update release: latest.json follows the Tauri static update format", () => {
  const text = release.latestJson({
    version: "0.2.0",
    notes: "- 자동 업데이트",
    pubDate: "2026-11-25T00:00:00.000Z",
    signature: "c2lnbmF0dXJl\n",
    arch: "arm64",
  });
  assert.deepEqual(JSON.parse(text), {
    version: "0.2.0",
    notes: "- 자동 업데이트",
    pub_date: "2026-11-25T00:00:00.000Z",
    platforms: {
      "darwin-aarch64": {
        signature: "c2lnbmF0dXJl",
        url: "https://github.com/gihwan-dev/stock-copilot/releases/download/v0.2.0/Stock.Copilot.app.tar.gz",
      },
    },
  });
  assert.equal(release.platformKey("x64"), "darwin-x86_64");
  assert.throws(() => release.platformKey("ia32"), release.ReleaseError);
  const conf = JSON.parse(
    readFileSync(
      join(repoRoot, "apps/desktop/src-tauri/tauri.conf.json"),
      "utf8",
    ),
  );
  assert.deepEqual(conf.plugins.updater.endpoints, [release.ENDPOINT]);
});

test("fin-update release: --dry-run prints every step without running or writing anything", () => {
  const before = snapshot();
  const result = runScript(["release", "0.2.0", "--dry-run"]);
  assert.equal(result.status, 0, result.stderr);
  const lines = result.stdout
    .split("\n")
    .filter((line) => line.startsWith("["));
  assert.equal(lines.length, release.releasePlan({ version: "0.2.0" }).length);
  for (const expected of [
    "git status --porcelain",
    "tauri build --bundles app",
    'APPLE_SIGNING_IDENTITY="Developer ID Application: GIHWAN CHOI (SLVS4WF9U2)"',
    "Contents/MacOS/gen-bindings",
    "target/release/mcp-bridge",
    "codesign --force --options runtime --timestamp --sign",
    "--entitlements apps/desktop/src-tauri/Entitlements.plist",
    "Stock.Copilot.app.tar.gz",
    "tauri signer sign",
    "latest.json",
    "gh release create v0.2.0",
  ]) {
    assert.ok(result.stdout.includes(expected), expected);
  }
  assert.ok(result.stdout.includes("TAURI_SIGNING_PRIVATE_KEY=<키체인>"));
  assert.ok(!/TAURI_SIGNING_PRIVATE_KEY=[^<]/u.test(result.stdout));
  assert.ok(
    result.stdout.indexOf("앱 서명") > result.stdout.indexOf("브리지 서명"),
  );
  assert.ok(
    result.stdout.indexOf("업데이트 묶음") > result.stdout.indexOf("서명 확인"),
  );
  assert.deepEqual(snapshot(), before);
});

test("fin-update release: init-key --dry-run lists the key steps and touches nothing", () => {
  const before = snapshot();
  for (const args of [
    ["init-key", "--dry-run"],
    ["--init-key", "--dry-run"],
  ]) {
    const result = runScript(args);
    assert.equal(result.status, 0, result.stderr);
    for (const expected of [
      "security find-generic-password -s dev.stockcopilot.updater-key",
      "tauri signer generate --ci",
      "security -i",
      "plugins.updater.pubkey",
    ]) {
      assert.ok(result.stdout.includes(expected), expected);
    }
  }
  assert.deepEqual(snapshot(), before);
});

test("fin-update release: a bad version or unknown command fails before any step", () => {
  for (const args of [
    ["release", "1.2", "--dry-run"],
    ["release", "--dry-run"],
    ["publish", "--dry-run"],
  ]) {
    const result = runScript(args);
    assert.equal(result.status, 1);
    assert.ok(!result.stdout.includes("[1/"));
  }
});
