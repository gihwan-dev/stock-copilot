import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { POLICY_FILE, buildCacheProblems } from "../../build-cache/check.mjs";
import { cacheStats, pruneCache } from "../../build-cache/prune.mjs";
import { parseInvocation } from "../../build-cache/rustc-wrapper.mjs";
import { childEnvironment, repoRoot, scratch } from "./signature-repo.mjs";

const WRAPPER_SH = join(repoRoot, "harness/build-cache/rustc-wrapper.sh");

const copyPolicyFixture = () => {
  const dir = scratch("h68-cache-policy-");
  for (const rel of [
    POLICY_FILE,
    "harness/build-cache/rustc-wrapper.sh",
    "harness/build-cache/rustc-wrapper.mjs",
    "harness/build-cache/prune.mjs",
    "justfile",
    "crates/xtask/src/coverage_check.rs",
    "crates/xtask/src/mutants_check.rs",
    "crates/xtask/src/mutants_check/catalog.rs",
  ]) {
    const dst = join(dir, rel);
    mkdirSync(join(dst, ".."), { recursive: true });
    cpSync(join(repoRoot, rel), dst);
  }
  return dir;
};

const setupSyntheticRegistryAndWorktree = (
  baseDir,
  worktreeName,
  crateReturnValue,
) => {
  const cargoHome = join(baseDir, "cargo-home");
  const depDir = join(
    cargoHome,
    "registry/src/index.crates.io-test/extdep-0.1.0",
  );
  mkdirSync(join(depDir, "src"), { recursive: true });
  if (!existsSync(join(depDir, "src/lib.rs"))) {
    writeFileSync(
      join(depDir, "src/lib.rs"),
      "pub fn ext_value() -> u32 { 41 }\n",
    );
  }
  const wt = join(baseDir, worktreeName);
  const outDir = join(wt, "target/debug/deps");
  mkdirSync(join(wt, "src"), { recursive: true });
  mkdirSync(outDir, { recursive: true });
  writeFileSync(
    join(wt, "src/lib.rs"),
    `pub fn ws_value() -> u32 { ${crateReturnValue} }\n`,
  );
  return { cargoHome, depDir, wt, outDir };
};

test("H68 build cache policy and wrapper isolate workspace coverage mutants and CI", () => {
  assert.deepEqual(buildCacheProblems(repoRoot), []);

  const badCoverage = copyPolicyFixture();
  try {
    const file = join(badCoverage, "crates/xtask/src/coverage_check.rs");
    writeFileSync(
      file,
      readFileSync(file, "utf8").replace(
        '.env("STOCK_COPILOT_BUILD_CACHE", "off")',
        "",
      ),
    );
    assert.ok(
      buildCacheProblems(badCoverage).some((item) =>
        item.includes("coverage_check.rs"),
      ),
    );
  } finally {
    rmSync(badCoverage, { recursive: true, force: true });
  }

  const badMutants = copyPolicyFixture();
  try {
    const file = join(badMutants, "crates/xtask/src/mutants_check.rs");
    writeFileSync(
      file,
      readFileSync(file, "utf8").replaceAll(
        '.env("STOCK_COPILOT_BUILD_CACHE", "off")',
        "",
      ),
    );
    assert.ok(
      buildCacheProblems(badMutants).some((item) =>
        item.includes("mutants_check.rs"),
      ),
    );
  } finally {
    rmSync(badMutants, { recursive: true, force: true });
  }

  const badWrapper = copyPolicyFixture();
  try {
    const sh = join(badWrapper, "harness/build-cache/rustc-wrapper.sh");
    writeFileSync(
      sh,
      readFileSync(sh, "utf8").replace("CARGO_PRIMARY_PACKAGE", "OTHER"),
    );
    assert.ok(
      buildCacheProblems(badWrapper).some((item) =>
        item.includes("CARGO_PRIMARY_PACKAGE"),
      ),
    );
  } finally {
    rmSync(badWrapper, { recursive: true, force: true });
  }

  const badPolicy = copyPolicyFixture();
  try {
    const policy = JSON.parse(
      readFileSync(join(badPolicy, POLICY_FILE), "utf8"),
    );
    policy.defaultMaxMegabytes = 10;
    writeFileSync(join(badPolicy, POLICY_FILE), `${JSON.stringify(policy)}\n`);
    assert.ok(
      buildCacheProblems(badPolicy).some((item) =>
        item.includes("defaultMaxMegabytes"),
      ),
    );
  } finally {
    rmSync(badPolicy, { recursive: true, force: true });
  }
});

test("H68 rustc-wrapper caches registry crates across worktrees blocks stale workspace artifacts and recovers from corruption", () => {
  const dir = scratch("h68-wrapper-e2e-");
  try {
    const cacheDir = join(dir, "shared-cache");
    const first = setupSyntheticRegistryAndWorktree(dir, "w1", 10);
    const second = setupSyntheticRegistryAndWorktree(dir, "w2", 20);
    const rustc = spawnSync("rustup", ["which", "rustc"], {
      encoding: "utf8",
      env: childEnvironment(),
    }).stdout.trim();
    assert.ok(rustc.endsWith("/rustc"), rustc);

    const runExt = (wt, outDir, extraEnv = {}) =>
      spawnSync(
        WRAPPER_SH,
        [
          rustc,
          "--crate-name",
          "extdep",
          "--edition=2024",
          join(first.depDir, "src/lib.rs"),
          "--crate-type",
          "lib",
          "--emit=dep-info,metadata,link",
          "-C",
          "metadata=aaa111",
          "-C",
          "extra-filename=-aaa111",
          "--out-dir",
          outDir,
        ],
        {
          cwd: first.depDir,
          encoding: "utf8",
          env: childEnvironment({
            CARGO_HOME: first.cargoHome,
            STOCK_COPILOT_BUILD_CACHE_DIR: cacheDir,
            CARGO_MANIFEST_DIR: first.depDir,
            CARGO_PKG_NAME: "extdep",
            CARGO_PKG_VERSION: "0.1.0",
            ...extraEnv,
          }),
        },
      );

    const cold = runExt(first.wt, first.outDir);
    assert.equal(cold.status, 0, cold.stderr);
    assert.ok(existsSync(join(first.outDir, "libextdep-aaa111.rlib")));
    const statsAfterCold = cacheStats(cacheDir, 4096);
    assert.equal(statsAfterCold.entries, 1);

    const warm = runExt(second.wt, second.outDir);
    assert.equal(warm.status, 0, warm.stderr);
    assert.ok(existsSync(join(second.outDir, "libextdep-aaa111.rlib")));
    const depText = readFileSync(
      join(second.outDir, "extdep-aaa111.d"),
      "utf8",
    );
    assert.ok(depText.includes(second.outDir), depText);
    assert.ok(!depText.includes(first.outDir), depText);

    const wsRun = spawnSync(
      WRAPPER_SH,
      [
        rustc,
        "--crate-name",
        "ws",
        "--edition=2024",
        join(first.wt, "src/lib.rs"),
        "--crate-type",
        "lib",
        "--emit=dep-info,metadata,link",
        "--out-dir",
        first.outDir,
      ],
      {
        cwd: first.wt,
        encoding: "utf8",
        env: childEnvironment({
          CARGO_HOME: first.cargoHome,
          STOCK_COPILOT_BUILD_CACHE_DIR: cacheDir,
        }),
      },
    );
    assert.equal(wsRun.status, 0, wsRun.stderr);
    assert.equal(cacheStats(cacheDir, 4096).entries, 1);

    const shardDir = readdirSync(join(cacheDir, "entries"))[0];
    const keyDir = readdirSync(join(cacheDir, "entries", shardDir))[0];
    const cachedRlib = join(
      cacheDir,
      "entries",
      shardDir,
      keyDir,
      "files",
      "libextdep-aaa111.rlib",
    );
    writeFileSync(cachedRlib, "corrupted-artifact");
    const third = setupSyntheticRegistryAndWorktree(dir, "w3", 30);
    const recovered = runExt(third.wt, third.outDir);
    assert.equal(recovered.status, 0, recovered.stderr);
    assert.notEqual(
      readFileSync(join(third.outDir, "libextdep-aaa111.rlib"), "utf8"),
      "corrupted-artifact",
    );

    const roCache = join(dir, "readonly-cache");
    mkdirSync(roCache, { recursive: true });
    chmodSync(roCache, 0o555);
    const fourth = setupSyntheticRegistryAndWorktree(dir, "w4", 40);
    try {
      const roRun = runExt(fourth.wt, fourth.outDir, {
        STOCK_COPILOT_BUILD_CACHE_DIR: roCache,
      });
      assert.equal(roRun.status, 0, roRun.stderr);
      assert.ok(existsSync(join(fourth.outDir, "libextdep-aaa111.rlib")));
    } finally {
      chmodSync(roCache, 0o755);
    }

    assert.equal(
      parseInvocation({
        rustc,
        args: [
          "--crate-name",
          "extdep",
          join(first.depDir, "src/lib.rs"),
          "--out-dir",
          first.outDir,
          "-C",
          "instrument-coverage",
        ],
        env: { CARGO_HOME: first.cargoHome },
        cwd: first.depDir,
      }),
      undefined,
    );
    assert.equal(
      parseInvocation({
        rustc,
        args: [
          "--crate-name",
          "extdep",
          join(first.depDir, "src/lib.rs"),
          "--out-dir",
          first.outDir,
        ],
        env: { CARGO_HOME: first.cargoHome, CI: "true" },
        cwd: first.depDir,
      }),
      undefined,
    );

    const outDirCache = join(dir, "outdir-cache");
    const genDepDir = join(
      first.cargoHome,
      "registry/src/index.crates.io-test/gendep-0.1.0",
    );
    mkdirSync(join(genDepDir, "src"), { recursive: true });
    writeFileSync(
      join(genDepDir, "src/lib.rs"),
      'include!(concat!(env!("OUT_DIR"), "/gen.rs"));\n',
    );
    const buildOut1 = join(first.wt, "target/debug/build/gendep-aaa/out");
    const buildOut2 = join(second.wt, "target/debug/build/gendep-aaa/out");
    mkdirSync(buildOut1, { recursive: true });
    mkdirSync(buildOut2, { recursive: true });
    writeFileSync(
      join(buildOut1, "gen.rs"),
      "pub fn gen_value() -> u32 { 7 }\n",
    );
    writeFileSync(
      join(buildOut2, "gen.rs"),
      "pub fn gen_value() -> u32 { 7 }\n",
    );
    const runGen = (outDir, buildOutDir, emit = "dep-info,metadata,link") =>
      spawnSync(
        WRAPPER_SH,
        [
          rustc,
          "--crate-name",
          "gendep",
          "--edition=2024",
          join(genDepDir, "src/lib.rs"),
          "--crate-type",
          "lib",
          `--emit=${emit}`,
          "-C",
          "metadata=bbb222",
          "-C",
          "extra-filename=-bbb222",
          "--out-dir",
          outDir,
        ],
        {
          cwd: genDepDir,
          encoding: "utf8",
          env: childEnvironment({
            CARGO_HOME: first.cargoHome,
            STOCK_COPILOT_BUILD_CACHE_DIR: outDirCache,
            CARGO_MANIFEST_DIR: genDepDir,
            CARGO_PKG_NAME: "gendep",
            CARGO_PKG_VERSION: "0.1.0",
            OUT_DIR: buildOutDir,
          }),
        },
      );
    const noDepInfo = runGen(first.outDir, buildOut1, "metadata,link");
    assert.equal(noDepInfo.status, 0, noDepInfo.stderr);
    assert.equal(cacheStats(outDirCache, 4096).entries, 0);

    const genCold = runGen(first.outDir, buildOut1);
    assert.equal(genCold.status, 0, genCold.stderr);
    assert.equal(cacheStats(outDirCache, 4096).entries, 1);

    const genWarm = runGen(second.outDir, buildOut2);
    assert.equal(genWarm.status, 0, genWarm.stderr);
    assert.equal(cacheStats(outDirCache, 4096).entries, 1);
    const genDepText = readFileSync(
      join(second.outDir, "gendep-bbb222.d"),
      "utf8",
    );
    assert.ok(genDepText.includes(buildOut2), genDepText);
    assert.ok(!genDepText.includes(buildOut1), genDepText);

    writeFileSync(
      join(buildOut2, "gen.rs"),
      "pub fn gen_value() -> u32 { 9 }\n",
    );
    const genChanged = runGen(second.outDir, buildOut2);
    assert.equal(genChanged.status, 0, genChanged.stderr);
    assert.equal(cacheStats(outDirCache, 4096).entries, 2);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("H68 cache prune enforces disk cap removes stale temps and protects active entries", () => {
  const dir = scratch("h68-cache-prune-");
  try {
    const cacheDir = join(dir, "cache");
    const makeEntry = (key, sizeBytes, ageSeconds) => {
      const entryDir = join(cacheDir, "entries", key.slice(0, 2), key);
      mkdirSync(join(entryDir, "files"), { recursive: true });
      writeFileSync(
        join(entryDir, "files/blob.rlib"),
        Buffer.alloc(sizeBytes, 1),
      );
      const manifest = join(entryDir, "manifest.json");
      writeFileSync(manifest, JSON.stringify({ key }));
      const past = new Date(Date.now() - ageSeconds * 1000);
      utimesSync(manifest, past, past);
      return entryDir;
    };
    const oldEntry = makeEntry("aa1111", 700 * 1024, 3600);
    const midEntry = makeEntry("bb2222", 700 * 1024, 1800);
    const hotEntry = makeEntry("cc3333", 700 * 1024, 10);
    const staleTmp = join(cacheDir, ".tmp/stale-job");
    mkdirSync(staleTmp, { recursive: true });
    const oldDate = new Date(Date.now() - 7200 * 1000);
    utimesSync(staleTmp, oldDate, oldDate);

    const pruned = pruneCache({
      cacheDir,
      maxMegabytes: 1,
      pruneTargetRatio: 0.8,
      minEntryAgeSeconds: 300,
      staleTempAgeSeconds: 1800,
    });
    assert.equal(pruned.removedTemps, 1);
    assert.equal(existsSync(staleTmp), false);
    assert.equal(existsSync(oldEntry), false);
    assert.equal(existsSync(midEntry), false);
    assert.equal(existsSync(hotEntry), true);

    const cleared = pruneCache({
      cacheDir,
      maxMegabytes: 1,
      pruneTargetRatio: 0.8,
      minEntryAgeSeconds: 300,
      staleTempAgeSeconds: 1800,
      clear: true,
    });
    assert.equal(cleared.afterEntries, 0);
    assert.equal(existsSync(hotEntry), false);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
