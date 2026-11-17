import assert from "node:assert/strict";
import fs from "node:fs";
import { execFileSync, spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");
const script = join(root, "harness/build-seed/seed.py");
const gitEnv = {
  ...process.env,
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_CONFIG_COUNT: "0",
  CARGO_BUILD_JOBS: "4",
  NEXTEST_TEST_THREADS: "4",
};

const fixture = (t) => {
  const parent = join(root, "target/H-51");
  fs.mkdirSync(parent, { recursive: true });
  const path = fs.mkdtempSync(join(parent, "seed-test-"));
  t.after(() => fs.rmSync(path, { force: true, recursive: true }));
  return path;
};

const git = (args) => execFileSync("git", args, { env: gitEnv, stdio: "pipe" });

const canClone = (directory) => {
  if (process.platform !== "darwin") return false;
  const source = join(directory, "clone-probe-source");
  const target = join(directory, "clone-probe-target");
  fs.writeFileSync(source, "probe");
  const python = [
    "import ctypes, os, sys",
    "clone = ctypes.CDLL(None, use_errno=True).clonefile",
    "clone.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int]",
    "clone.restype = ctypes.c_int",
    "sys.exit(0 if clone(os.fsencode(sys.argv[1]), os.fsencode(sys.argv[2]), 1) == 0 else 1)",
  ].join("\n");
  const run = spawnSync(
    "/usr/bin/python3",
    ["-I", "-B", "-c", python, source, target],
    {
      encoding: "utf8",
    },
  );
  fs.rmSync(source, { force: true });
  fs.rmSync(target, { force: true });
  return (
    run.status === 0 &&
    fs.existsSync(join(process.env.HOME, ".cargo/bin/cargo")) &&
    fs.existsSync(join(process.env.HOME, ".cargo/bin/rustc"))
  );
};

const worktrees = (t) => {
  const directory = fixture(t);
  const main = join(directory, "main");
  const work = join(directory, "work");
  fs.mkdirSync(main);
  git(["init", "-q", main]);
  git([
    "-C",
    main,
    "-c",
    "user.name=H51",
    "-c",
    "user.email=h51@example.invalid",
    "-c",
    "commit.gpgsign=false",
    "commit",
    "-q",
    "--allow-empty",
    "-m",
    "fixture",
  ]);
  git(["-C", main, "worktree", "add", "-q", "-b", "h51-fixture", work]);
  fs.writeFileSync(
    join(work, "Cargo.toml"),
    '[package]\nname = "h51-workspace"\nversion = "0.1.0"\nedition = "2024"\n[workspace]\n',
  );
  fs.mkdirSync(join(work, "src"));
  fs.writeFileSync(join(work, "src/lib.rs"), "pub fn value() -> u32 { 7 }\n");
  const locked = spawnSync("cargo", ["generate-lockfile", "--offline"], {
    cwd: work,
    env: { ...gitEnv, CARGO_HOME: join(directory, "cargo-home") },
    encoding: "utf8",
  });
  assert.equal(locked.status, 0, locked.stderr);
  return { directory, main, work };
};

const runSeed = (work, source) =>
  spawnSync("/usr/bin/python3", ["-I", script], {
    cwd: work,
    env: {
      ...gitEnv,
      CARGO_HOME: join(dirname(work), "cargo-home"),
      STOCK_COPILOT_SEED_TARGET: source,
    },
    encoding: "utf8",
  });

const seed = (main) => {
  const directory = join(main, "target/debug/deps");
  fs.mkdirSync(directory, { recursive: true });
  fs.writeFileSync(
    join(main, "target/CACHEDIR.TAG"),
    "Signature: 8a477f597d28d172789f06886806bc55\n",
  );
  fs.writeFileSync(join(directory, "libthirdparty.rlib"), "trusted seed");
  return join(main, "target");
};

test("a missing seed leaves target absent and succeeds", (t) => {
  const { main, work } = worktrees(t);
  const result = runSeed(work, join(main, "target"));
  assert.equal(result.status, 0, result.stderr);
  assert.equal(fs.existsSync(join(work, "target")), false);
});

test("a nonempty target and an untrusted seed path are never modified", (t) => {
  const { directory, main, work } = worktrees(t);
  const source = seed(main);
  const target = join(work, "target");
  fs.mkdirSync(target);
  fs.writeFileSync(join(target, "keep"), "original");
  let result = runSeed(work, source);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(fs.readdirSync(target), ["keep"]);
  fs.rmSync(target, { recursive: true });
  const other = join(directory, "other");
  fs.mkdirSync(other);
  fs.writeFileSync(join(other, "wrong"), "wrong");
  result = runSeed(work, other);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(fs.existsSync(target), false);
  assert.equal(fs.readFileSync(join(other, "wrong"), "utf8"), "wrong");
});

test("a clone is private, preserves the seed, and omits symlinks", (t) => {
  const { directory, main, work } = worktrees(t);
  const supported = canClone(directory);
  const source = seed(main);
  const sourceFile = join(source, "debug/deps/libthirdparty.rlib");
  fs.symlinkSync(sourceFile, join(source, "debug/deps/alias"));
  fs.mkdirSync(join(work, "target"));
  const before = fs.statSync(sourceFile);
  const result = runSeed(work, source);
  assert.equal(result.status, 0, result.stderr);
  const clone = join(work, "target/debug/deps/libthirdparty.rlib");
  if (!supported) {
    assert.deepEqual(fs.readdirSync(join(work, "target")), []);
    assert.equal(fs.readFileSync(sourceFile, "utf8"), "trusted seed");
    return;
  }
  assert.match(result.stdout, /복제 완료/u);
  assert.ok(fs.existsSync(clone), result.stdout + result.stderr);
  assert.equal(fs.readFileSync(clone, "utf8"), "trusted seed");
  assert.notEqual(fs.statSync(clone).ino, before.ino);
  assert.equal(fs.existsSync(join(work, "target/debug/deps/alias")), false);
  fs.writeFileSync(clone, "private change");
  assert.equal(fs.readFileSync(sourceFile, "utf8"), "trusted seed");
  assert.equal(fs.statSync(sourceFile).mtimeMs, before.mtimeMs);
});

test("failed cloning leaves the target empty and never falls back to copying", (t) => {
  const directory = fixture(t);
  const source = join(directory, "source");
  const target = join(directory, "destination");
  fs.mkdirSync(source);
  fs.mkdirSync(target);
  fs.writeFileSync(join(source, "artifact"), "original");
  const python = [
    "import errno, importlib.util, sys",
    "spec = importlib.util.spec_from_file_location('seed', sys.argv[1])",
    "module = importlib.util.module_from_spec(spec)",
    "spec.loader.exec_module(module)",
    "module.sys.platform = 'darwin'",
    "def fail(source, target):",
    "    raise OSError(errno.ENOTSUP, 'clone unsupported')",
    "module.clone_file = fail",
    "module.seed_directory(sys.argv[2], sys.argv[3], lambda _stage: None)",
  ].join("\n");
  const run = spawnSync(
    "/usr/bin/python3",
    ["-I", "-B", "-c", python, script, source, target],
    { encoding: "utf8" },
  );
  assert.equal(run.status, 0, run.stderr);
  assert.deepEqual(fs.readdirSync(target), []);
  assert.equal(fs.readFileSync(join(source, "artifact"), "utf8"), "original");
  assert.deepEqual(fs.readdirSync(directory).sort(), ["destination", "source"]);
});

test("fresh-cargo removes compiled non-workspace dependencies", (t) => {
  const directory = fixture(t);
  const workspace = join(directory, "workspace");
  const dependency = join(directory, "dependency");
  fs.mkdirSync(join(workspace, "src"), { recursive: true });
  fs.mkdirSync(join(dependency, "src"), { recursive: true });
  fs.writeFileSync(
    join(workspace, "Cargo.toml"),
    '[package]\nname = "h51-probe"\nversion = "0.1.0"\nedition = "2024"\n[dependencies]\nh51-dependency = { path = "../dependency" }\n[workspace]\n',
  );
  fs.writeFileSync(
    join(workspace, "src/lib.rs"),
    "pub fn value() -> u32 { h51_dependency::value() }\n",
  );
  fs.writeFileSync(
    join(dependency, "Cargo.toml"),
    '[package]\nname = "h51-dependency"\nversion = "0.1.0"\nedition = "2024"\n[workspace]\n',
  );
  fs.writeFileSync(
    join(dependency, "src/lib.rs"),
    "pub fn value() -> u32 { 7 }\n",
  );
  const cargoEnv = { ...gitEnv, CARGO_HOME: join(directory, "cargo-home") };
  for (const args of [
    ["generate-lockfile", "--offline"],
    ["build", "--locked", "--offline"],
  ]) {
    const run = spawnSync("cargo", args, {
      cwd: workspace,
      env: cargoEnv,
      encoding: "utf8",
    });
    assert.equal(run.status, 0, run.stderr);
  }
  const deps = join(workspace, "target/debug/deps");
  assert.ok(
    fs
      .readdirSync(deps)
      .some((name) => /^libh51_dependency-.*\.rlib$/u.test(name)),
  );
  const run = spawnSync(
    "just",
    [
      "--justfile",
      join(root, "justfile"),
      "--working-directory",
      workspace,
      "fresh-cargo",
    ],
    { cwd: workspace, env: cargoEnv, encoding: "utf8" },
  );
  assert.equal(run.status, 0, run.stderr);
  assert.equal(
    fs.existsSync(deps) &&
      fs
        .readdirSync(deps)
        .some((name) => /^libh51_dependency-.*\.rlib$/u.test(name)),
    false,
  );
});

test("seeded and unseeded workspaces rebuild changed code and agree", (t) => {
  const { directory, main, work } = worktrees(t);
  const supported = canClone(directory);
  const manifest = fs.readFileSync(join(work, "Cargo.toml"));
  fs.writeFileSync(join(main, "Cargo.toml"), manifest);
  fs.mkdirSync(join(main, "src"));
  fs.copyFileSync(join(work, "src/lib.rs"), join(main, "src/lib.rs"));
  const oldLibTime = fs.statSync(join(main, "src/lib.rs")).mtime;
  fs.utimesSync(join(work, "src/lib.rs"), oldLibTime, oldLibTime);
  const oldSource = 'fn main() { println!("old"); }\n';
  const newSource = 'fn main() { println!("new"); }\n';
  fs.writeFileSync(join(main, "src/main.rs"), oldSource);
  fs.writeFileSync(join(work, "src/main.rs"), newSource);
  const oldTime = fs.statSync(join(main, "src/main.rs")).mtime;
  fs.utimesSync(join(work, "src/main.rs"), oldTime, oldTime);
  const cargoEnv = { ...gitEnv, CARGO_HOME: join(directory, "cargo-home") };
  const cargo = (cwd, args) =>
    spawnSync("cargo", args, { cwd, env: cargoEnv, encoding: "utf8" });
  const locked = cargo(main, ["generate-lockfile", "--offline"]);
  assert.equal(locked.status, 0, locked.stderr);
  const initial = cargo(main, ["run", "--locked", "--offline", "--quiet"]);
  assert.equal(initial.status, 0, initial.stderr);
  assert.equal(initial.stdout.trim(), "old");
  const control = join(directory, "control");
  fs.mkdirSync(join(control, "src"), { recursive: true });
  fs.copyFileSync(join(work, "Cargo.toml"), join(control, "Cargo.toml"));
  fs.copyFileSync(join(work, "Cargo.lock"), join(control, "Cargo.lock"));
  fs.writeFileSync(join(control, "src/main.rs"), newSource);
  fs.writeFileSync(
    join(control, "src/lib.rs"),
    "pub fn value() -> u32 { 7 }\n",
  );
  const seeded = runSeed(work, join(main, "target"));
  assert.equal(seeded.status, 0, seeded.stderr);
  assert.match(seeded.stdout, supported ? /복제 완료/u : /복제 건너뜀/u);
  const seededRun = cargo(work, ["run", "--locked", "--offline", "--quiet"]);
  const controlRun = cargo(control, [
    "run",
    "--locked",
    "--offline",
    "--quiet",
  ]);
  assert.equal(seededRun.status, 0, seededRun.stderr);
  assert.equal(controlRun.status, 0, controlRun.stderr);
  assert.equal(seededRun.stdout.trim(), "new");
  assert.equal(seededRun.stdout, controlRun.stdout);
  const seededTest = cargo(work, ["test", "--locked", "--offline", "--quiet"]);
  const controlTest = cargo(control, [
    "test",
    "--locked",
    "--offline",
    "--quiet",
  ]);
  assert.equal(seededTest.status, controlTest.status);
  assert.equal(seededTest.status, 0, seededTest.stderr);
  assert.equal(fs.readFileSync(join(main, "src/main.rs"), "utf8"), oldSource);
});

test("fresh and CI entrypoints never request a seed", () => {
  const dumped = spawnSync("just", ["--dump", "--dump-format", "json"], {
    cwd: root,
    env: gitEnv,
    encoding: "utf8",
  });
  assert.equal(dumped.status, 0, dumped.stderr);
  const recipes = JSON.parse(dumped.stdout).recipes;
  for (const name of [
    "check",
    "check-full",
    "check-fast",
    "fresh-build",
    "fresh-cargo",
    "fresh-node",
    "linux-check",
    "linux-test",
    "mutants-diff",
    "mutants-daily",
    "mutants-all",
  ]) {
    assert.ok(recipes[name], name);
    assert.doesNotMatch(
      JSON.stringify(recipes[name]),
      /seed-target|seed\.py/u,
      name,
    );
  }
  for (const name of ["ci.yml", "nightly.yml"]) {
    const workflow = fs.readFileSync(
      join(root, ".github/workflows", name),
      "utf8",
    );
    assert.doesNotMatch(workflow, /seed-target|seed\.py/u, name);
  }
});

test("a different lockfile rebuilds dependencies with the same result without a seed", (t) => {
  const { directory, main, work } = worktrees(t);
  const supported = canClone(directory);
  for (const [name, version, value] of [
    ["dep-one", "1.0.0", "1"],
    ["dep-two", "2.0.0", "2"],
  ]) {
    const dep = join(directory, name);
    fs.mkdirSync(join(dep, "src"), { recursive: true });
    fs.writeFileSync(
      join(dep, "Cargo.toml"),
      `[package]\nname = "h51-versioned-dep"\nversion = "${version}"\nedition = "2024"\n[workspace]\n`,
    );
    fs.writeFileSync(
      join(dep, "src/lib.rs"),
      `pub fn value() -> u32 { ${value} }\n`,
    );
  }
  const manifest = (dep) =>
    `[package]\nname = "h51-version-probe"\nversion = "0.1.0"\nedition = "2024"\n[dependencies]\nh51-versioned-dep = { path = "../${dep}" }\n[workspace]\n`;
  const library =
    "pub fn value() -> u32 { h51_versioned_dep::value() }\n#[cfg(test)] mod tests { #[test] fn value() { assert_eq!(super::value(), 2); } }\n";
  fs.mkdirSync(join(main, "src"));
  fs.writeFileSync(join(main, "Cargo.toml"), manifest("dep-one"));
  fs.writeFileSync(join(work, "Cargo.toml"), manifest("dep-two"));
  fs.writeFileSync(join(main, "src/lib.rs"), library);
  fs.writeFileSync(join(work, "src/lib.rs"), library);
  const control = join(directory, "control");
  fs.mkdirSync(join(control, "src"), { recursive: true });
  fs.writeFileSync(join(control, "Cargo.toml"), manifest("dep-two"));
  fs.writeFileSync(join(control, "src/lib.rs"), library);
  const cargoEnv = { ...gitEnv, CARGO_HOME: join(directory, "cargo-home") };
  const cargo = (cwd, args) =>
    spawnSync("cargo", args, { cwd, env: cargoEnv, encoding: "utf8" });
  for (const dir of [main, work, control]) {
    const lock = cargo(dir, ["generate-lockfile", "--offline"]);
    assert.equal(lock.status, 0, lock.stderr);
  }
  assert.match(
    fs.readFileSync(join(main, "Cargo.lock"), "utf8"),
    /version = "1\.0\.0"/u,
  );
  assert.match(
    fs.readFileSync(join(work, "Cargo.lock"), "utf8"),
    /version = "2\.0\.0"/u,
  );
  const initial = cargo(main, ["build", "--locked", "--offline", "--quiet"]);
  assert.equal(initial.status, 0, initial.stderr);
  const seedDeps = join(main, "target/debug/deps");
  const artifactName = fs
    .readdirSync(seedDeps)
    .find((name) => /^libh51_versioned_dep-.*\.rlib$/u.test(name));
  assert.ok(artifactName);
  const artifact = join(seedDeps, artifactName);
  const before = fs.readFileSync(artifact);
  const beforeTime = fs.statSync(artifact).mtimeMs;
  const seeded = runSeed(work, join(main, "target"));
  assert.equal(seeded.status, 0, seeded.stderr);
  assert.match(seeded.stdout, supported ? /복제 완료/u : /복제 건너뜀/u);
  const withSeed = cargo(work, ["test", "--locked", "--offline", "--quiet"]);
  const withoutSeed = cargo(control, [
    "test",
    "--locked",
    "--offline",
    "--quiet",
  ]);
  assert.equal(withSeed.status, 0, withSeed.stderr);
  assert.equal(withoutSeed.status, 0, withoutSeed.stderr);
  assert.equal(withSeed.status, withoutSeed.status);
  assert.deepEqual(fs.readFileSync(artifact), before);
  assert.equal(fs.statSync(artifact).mtimeMs, beforeTime);
});

test("a target file or a target inside the seed is never replaced", (t) => {
  const { directory, main, work } = worktrees(t);
  const source = seed(main);
  const destination = join(work, "target");
  fs.writeFileSync(destination, "keep");
  const run = runSeed(work, source);
  assert.equal(run.status, 0, run.stderr);
  assert.equal(fs.readFileSync(destination, "utf8"), "keep");
  const overlap = join(directory, "overlap");
  fs.mkdirSync(join(overlap, "nested"), { recursive: true });
  const python = [
    "import importlib.util, json, sys",
    "spec = importlib.util.spec_from_file_location('seed', sys.argv[1])",
    "m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)",
    "m.sys.platform = 'darwin'",
    "called = []",
    "def attempt(source, target):",
    "    called.append(True); raise OSError('must not clone')",
    "m.clone_tree = attempt",
    "result = m.seed_directory(sys.argv[2], sys.argv[3], lambda _stage: None)",
    "print(json.dumps([result, called, m.empty_destination(sys.argv[4])]))",
  ].join("\n");
  const check = spawnSync(
    "/usr/bin/python3",
    [
      "-I",
      "-B",
      "-c",
      python,
      script,
      overlap,
      join(overlap, "nested", "target"),
      destination,
    ],
    { encoding: "utf8" },
  );
  assert.equal(check.status, 0, check.stderr);
  assert.deepEqual(JSON.parse(check.stdout), [false, [], false]);
});

test("workspace cleanup is locked, offline, and limited to the cloned target", (t) => {
  const directory = fixture(t);
  const python = [
    "import importlib.util, json, sys",
    "spec = importlib.util.spec_from_file_location('seed', sys.argv[1])",
    "m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)",
    "calls = []",
    "def record(args, **kwargs):",
    "    calls.append([args, kwargs['cwd'], kwargs['check']])",
    "m.subprocess.run = record",
    "m.clear_workspace(sys.argv[2], sys.argv[3])",
    "print(json.dumps(calls))",
  ].join("\n");
  const work = join(directory, "work");
  const stage = join(directory, "stage");
  const run = spawnSync(
    "/usr/bin/python3",
    ["-I", "-B", "-c", python, script, work, stage],
    { encoding: "utf8" },
  );
  assert.equal(run.status, 0, run.stderr);
  assert.deepEqual(JSON.parse(run.stdout), [
    [
      [
        join(process.env.HOME, ".cargo/bin/cargo"),
        "clean",
        "--locked",
        "--offline",
        "--workspace",
        "--target-dir",
        stage,
      ],
      work,
      true,
    ],
  ]);
});

test("a cloned symlink is rejected even if the source changes during cloning", (t) => {
  const directory = fixture(t);
  const source = join(directory, "source");
  const target = join(directory, "target");
  fs.writeFileSync(source, "seed");
  const python = [
    "import importlib.util, os, sys",
    "spec = importlib.util.spec_from_file_location('seed', sys.argv[1])",
    "m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)",
    "class Clone:",
    "    def __call__(self, source, target, flags):",
    "        assert flags == 1",
    "        os.symlink(os.fsdecode(source), os.fsdecode(target)); return 0",
    "class Library:",
    "    clonefile = Clone()",
    "m.ctypes.CDLL = lambda *_args, **_kwargs: Library()",
    "try:",
    "    m.clone_file(sys.argv[2], sys.argv[3])",
    "except OSError:",
    "    print('rejected')",
    "else:",
    "    raise SystemExit('symlink accepted')",
  ].join("\n");
  const run = spawnSync(
    "/usr/bin/python3",
    ["-I", "-B", "-c", python, script, source, target],
    { encoding: "utf8" },
  );
  assert.equal(run.status, 0, run.stderr);
  assert.equal(run.stdout.trim(), "rejected");
  assert.equal(fs.readFileSync(source, "utf8"), "seed");
});

test("a symlink seed root and a missing seed do not start cloning", (t) => {
  const { directory, main, work } = worktrees(t);
  const realSeed = join(directory, "real-seed");
  fs.mkdirSync(realSeed);
  fs.writeFileSync(join(realSeed, "artifact"), "keep");
  fs.mkdirSync(join(realSeed, "debug/deps"), { recursive: true });
  fs.writeFileSync(join(realSeed, "debug/deps/libthirdparty.rlib"), "seed");
  fs.writeFileSync(
    join(realSeed, "CACHEDIR.TAG"),
    "Signature: 8a477f597d28d172789f06886806bc55\n",
  );
  fs.symlinkSync(realSeed, join(main, "target"));
  const symlink = runSeed(work, join(main, "target"));
  assert.equal(symlink.status, 0, symlink.stderr);
  assert.equal(fs.existsSync(join(work, "target")), false);
  assert.equal(fs.readFileSync(join(realSeed, "artifact"), "utf8"), "keep");
  const python = [
    "import importlib.util, json, sys",
    "spec = importlib.util.spec_from_file_location('seed', sys.argv[1])",
    "m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)",
    "m.sys.platform = 'darwin'",
    "called = []",
    "def attempt(source, target):",
    "    called.append(True); raise OSError('must not clone')",
    "m.clone_tree = attempt",
    "result = m.seed_directory(sys.argv[2], sys.argv[3], lambda _stage: None)",
    "print(json.dumps([result, called]))",
  ].join("\n");
  const check = spawnSync(
    "/usr/bin/python3",
    [
      "-I",
      "-B",
      "-c",
      python,
      script,
      join(directory, "missing"),
      join(directory, "new-target"),
    ],
    { encoding: "utf8" },
  );
  assert.equal(check.status, 0, check.stderr);
  assert.deepEqual(JSON.parse(check.stdout), [false, []]);
});

test("a failed workspace cleanup discards the private clone", (t) => {
  const directory = fixture(t);
  const source = join(directory, "source");
  const target = join(directory, "target");
  fs.mkdirSync(source);
  fs.mkdirSync(target);
  fs.writeFileSync(join(source, "artifact"), "seed");
  const python = [
    "import importlib.util, json, pathlib, subprocess, sys",
    "spec = importlib.util.spec_from_file_location('seed', sys.argv[1])",
    "m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)",
    "m.sys.platform = 'darwin'",
    "def clone(source, target):",
    "    pathlib.Path(target, 'artifact').write_text('seed')",
    "m.clone_tree = clone",
    "def fail(stage):",
    "    raise subprocess.CalledProcessError(1, 'cargo')",
    "print(json.dumps(m.seed_directory(sys.argv[2], sys.argv[3], fail)))",
  ].join("\n");
  const run = spawnSync(
    "/usr/bin/python3",
    ["-I", "-B", "-c", python, script, source, target],
    { encoding: "utf8" },
  );
  assert.equal(run.status, 0, run.stderr);
  assert.equal(JSON.parse(run.stdout), false);
  assert.deepEqual(fs.readdirSync(target), []);
  assert.equal(fs.readFileSync(join(source, "artifact"), "utf8"), "seed");
  assert.deepEqual(fs.readdirSync(directory).sort(), ["source", "target"]);
});

test("a fake cargo at the front of PATH cannot approve a seed", (t) => {
  const { directory, main, work } = worktrees(t);
  const supported = canClone(directory);
  const source = seed(main);
  const fakeBin = join(directory, "fake-bin");
  const marker = join(directory, "fake-cargo-ran");
  fs.mkdirSync(fakeBin);
  fs.writeFileSync(
    join(fakeBin, "cargo"),
    `#!/bin/sh\nprintf hit > '${marker}'\nexit 0\n`,
  );
  fs.chmodSync(join(fakeBin, "cargo"), 0o755);
  const run = spawnSync("/usr/bin/python3", ["-I", script], {
    cwd: work,
    env: {
      ...gitEnv,
      CARGO_HOME: join(directory, "cargo-home"),
      PATH: `${fakeBin}:${process.env.PATH}`,
      STOCK_COPILOT_SEED_TARGET: source,
    },
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  assert.equal(fs.existsSync(marker), false);
  assert.match(run.stdout, supported ? /복제 완료/u : /복제 건너뜀/u);
});

test("a cargo-home config cannot replace the cleanup compiler", (t) => {
  const { directory, work } = worktrees(t);
  const cargoHome = join(directory, "cargo-home");
  const fake = join(directory, "fake-rustc");
  const marker = join(directory, "fake-rustc-ran");
  const stage = join(directory, "stage");
  fs.mkdirSync(stage);
  fs.writeFileSync(
    join(stage, "CACHEDIR.TAG"),
    "Signature: 8a477f597d28d172789f06886806bc55\n",
  );
  fs.writeFileSync(fake, `#!/bin/sh\nprintf hit > '${marker}'\nexit 1\n`, {
    mode: 0o755,
  });
  fs.writeFileSync(
    join(cargoHome, "config.toml"),
    `[build]\nrustc = "${fake}"\nrustc-wrapper = "${fake}"\n`,
  );
  const python = [
    "import importlib.util, sys",
    "spec = importlib.util.spec_from_file_location('seed', sys.argv[1])",
    "m = importlib.util.module_from_spec(spec); spec.loader.exec_module(m)",
    "m.clear_workspace(sys.argv[2], sys.argv[3])",
  ].join("\n");
  const run = spawnSync(
    "/usr/bin/python3",
    ["-I", "-B", "-c", python, script, work, stage],
    {
      env: {
        ...gitEnv,
        CARGO_HOME: cargoHome,
        RUSTC: fake,
        RUSTC_WRAPPER: fake,
        RUSTC_WORKSPACE_WRAPPER: fake,
        CARGO_BUILD_RUSTC: fake,
        CARGO_BUILD_RUSTC_WRAPPER: fake,
        CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER: fake,
      },
      encoding: "utf8",
    },
  );
  assert.equal(run.status, 0, run.stderr);
  assert.equal(fs.existsSync(marker), false);
});

test("just seed-target uses the configured main target", (t) => {
  const { directory, main, work } = worktrees(t);
  const supported = canClone(directory);
  const source = seed(main);
  const localScript = join(work, "harness/build-seed/seed.py");
  fs.mkdirSync(dirname(localScript), { recursive: true });
  fs.copyFileSync(script, localScript);
  const run = spawnSync(
    "just",
    [
      "--justfile",
      join(root, "justfile"),
      "--working-directory",
      work,
      "seed-target",
    ],
    {
      cwd: work,
      env: {
        ...gitEnv,
        CARGO_HOME: join(directory, "cargo-home"),
        STOCK_COPILOT_SEED_TARGET: source,
      },
      encoding: "utf8",
    },
  );
  assert.equal(run.status, 0, run.stderr);
  if (supported) {
    assert.equal(
      fs.readFileSync(
        join(work, "target/debug/deps/libthirdparty.rlib"),
        "utf8",
      ),
      "trusted seed",
    );
  } else {
    assert.equal(fs.existsSync(join(work, "target")), false);
  }
  assert.equal(
    fs.readFileSync(join(source, "debug/deps/libthirdparty.rlib"), "utf8"),
    "trusted seed",
  );
});

test("recipes fix nextest threads at four while discarding caller overrides", (t) => {
  const run = spawnSync("just", ["--dump", "--dump-format", "json"], {
    cwd: root,
    env: { ...gitEnv, NEXTEST_TEST_THREADS: "99" },
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stderr);
  const dump = JSON.parse(run.stdout);
  assert.ok(dump.unexports.includes("NEXTEST_TEST_THREADS"));
  for (const name of ["check", "check-ts", "check-fast", "coverage-check"]) {
    assert.ok(
      dump.recipes[name].attributes.some(
        (item) =>
          JSON.stringify(item) ===
          JSON.stringify({ env: ["NEXTEST_TEST_THREADS", "4"] }),
      ),
      name,
    );
  }
  const justfile = join(fixture(t), "probe.just");
  fs.writeFileSync(
    justfile,
    `${fs.readFileSync(join(root, "justfile"), "utf8")}\n[env("NEXTEST_TEST_THREADS", "4")]\nh51-thread-probe:\n    printf '%s' "$NEXTEST_TEST_THREADS"\n`,
  );
  const probe = spawnSync(
    "just",
    ["--justfile", justfile, "--working-directory", root, "h51-thread-probe"],
    {
      cwd: root,
      env: { ...gitEnv, NEXTEST_TEST_THREADS: "99" },
      encoding: "utf8",
    },
  );
  assert.equal(probe.status, 0, probe.stderr);
  assert.equal(probe.stdout, "4");
});
