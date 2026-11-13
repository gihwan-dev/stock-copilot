import { strict as assert } from "node:assert";
import {
  readFileSync,
  cpSync,
  mkdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { test } from "node:test";
import { nodeFixture } from "../../git-hooks/node-fixture.mjs";
import {
  installPolicyProblems,
  TRUSTED_LOCKFILE_FIXTURES,
} from "../../ts-deps/rules.mjs";
import { repoRoot, createFixture } from "./fixture.mjs";
import { join, dirname } from "node:path";

test("offline Node fixture follows the manifest pin and rejects missing lock elements", () => {
  const manifest = JSON.parse(
    readFileSync(join(repoRoot, "package.json"), "utf8"),
  );
  const lock = readFileSync(join(repoRoot, "pnpm-lock.yaml"), "utf8");
  const version = manifest.devDependencies.prettier;
  const fixture = nodeFixture(manifest, lock);
  assert.equal(
    fixture.lock.importers["."].devDependencies.prettier.version,
    version,
  );
  assert.equal(fixture.manifest.devDependencies.prettier, version);
  assert.equal(fixture.manifest.packageManager, manifest.packageManager);
  assert.equal(fixture.managerLock, lock.slice(0, lock.indexOf("\n---\n") + 1));
  assert.throws(() => nodeFixture(manifest, lock.replaceAll("\n---\n", "\n")));
  const integrity =
    fixture.lock.packages["prettier@" + version].resolution.integrity;
  assert.ok(lock.includes(integrity));
  for (const line of [
    "        version: " + version,
    "  prettier@" + version + ":",
    "    hasBin: true",
    "  prettier@" + version + ": {}",
  ])
    assert.throws(() =>
      nodeFixture(
        manifest,
        lock
          .split("\n")
          .filter((entry) => entry !== line)
          .join("\n"),
      ),
    );
  assert.throws(() =>
    nodeFixture(manifest, lock.replace(integrity, "missing")),
  );
  const changed = structuredClone(manifest);
  changed.devDependencies.prettier = "99.0.0";
  const upgraded = nodeFixture(changed, lock.replaceAll(version, "99.0.0"));
  assert.equal(
    upgraded.lock.packages["prettier@99.0.0"].resolution.integrity,
    integrity,
  );
});
test("ts-deps entry point rejects policy skipping in a tracked real install command", () => {
  const f = createFixture();
  try {
    for (const name of [
      "package.json",
      "pnpm-lock.yaml",
      "pnpm-workspace.yaml",
      ".npmrc",
      "apps/desktop/package.json",
    ]) {
      mkdirSync(dirname(join(f.root, name)), { recursive: true });
      cpSync(join(repoRoot, name), join(f.root, name));
      assert.equal(spawnSync("git", ["add", name], { cwd: f.root }).status, 0);
    }
    const run = () =>
      spawnSync(
        process.execPath,
        [join(repoRoot, "harness/ts-deps/check.mjs"), f.root],
        { encoding: "utf8" },
      );
    const clean = run();
    assert.equal(clean.status, 0, clean.stdout + clean.stderr);
    for (const path of ["justfile", "harness/install.md"]) {
      writeFileSync(join(f.root, path), "pnpm install --trust-" + "lockfile\n");
      assert.equal(spawnSync("git", ["add", path], { cwd: f.root }).status, 0);
      const bad = run();
      assert.equal(bad.status, 1, bad.stdout + bad.stderr);
      assert.ok(bad.stdout.includes(path + ": fixture 밖의 공급망 정책"));
      writeFileSync(join(f.root, path), "fixture\n");
    }
  } finally {
    rmSync(f.base, { recursive: true, force: true });
  }
});
test("trust lockfile flag is confined to the approved restoration fixture and removing its exception fails", () => {
  const fixture = "crates/xtask/src/git_hooks/tests.rs";
  assert.deepEqual(TRUSTED_LOCKFILE_FIXTURES, [fixture]);
  const command = "pnpm install --trust-" + "lockfile";
  for (const path of [
    "justfile",
    "harness/ci/tools.mjs",
    ".claude/hooks/run.mjs",
    ".codex/run.sh",
    ".github/workflows/ci.yml",
    "package.json",
  ])
    assert.deepEqual(installPolicyProblems({ [path]: command }), [
      path + ": fixture 밖의 공급망 정책 검증 생략 옵션",
    ]);
  assert.deepEqual(installPolicyProblems({ [fixture]: command }), []);
  for (const text of [
    "pnpm_config_trust_" + "lockfile=true",
    "PNPM_CONFIG_TRUST_" + "LOCKFILE=true",
    "trustLock" + "file: true",
    "trust-" + "lockfile=true",
    "--config.trust-" + "lockfile=true",
    "--config.trust-" + "lockfile true",
  ])
    assert.equal(
      installPolicyProblems({ "harness/ci/tools.mjs": text }).length,
      1,
    );
  assert.equal(installPolicyProblems({ [fixture]: command }, []).length, 1);
  assert.deepEqual(
    installPolicyProblems({
      justfile: "pnpm install --frozen-lockfile --ignore-scripts",
    }),
    [],
  );
});
