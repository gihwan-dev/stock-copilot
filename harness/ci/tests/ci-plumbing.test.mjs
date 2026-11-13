import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { ZERO, changedRecently, rangeFor } from "../ci-range.mjs";
import {
  cargoArgs,
  exclusionProblems,
  nextestFilter,
  readExclusions,
  readMetadata,
} from "../linux/exclusions.mjs";
import { INNER_SCRIPT, offlineCommand } from "../linux/offline.mjs";
import { VERIFIER, runFromBase } from "../signatures/from-base.mjs";
import {
  installActionSpec,
  lefthookAsset,
  prefetchDependencies,
  prefetchManifest,
  readTools,
} from "../tools.mjs";
import {
  commit,
  createRepository,
  removeRepository,
  repoRoot,
} from "./signature-repo.mjs";

const workflow = (name) =>
  JSON.parse(
    readFileSync(join(repoRoot, ".github", "workflows", name), "utf8"),
  );

const runs = (job) => job.steps.map((step) => step.run ?? step.uses);

test("the Linux exclusion list holds exactly the reviewed macOS-only crates and tests", () => {
  const exclusions = readExclusions(repoRoot);
  assert.deepEqual(
    exclusions.crates.map((entry) => entry.name),
    ["src-tauri", "e2e-app", "platform-macos", "secrets-keychain"],
  );
  assert.deepEqual(
    exclusions.tests.map((entry) => `${entry.package}::${entry.module}`),
    ["xtask::network_sandbox_tests", "xtask::nested_sandbox_tests"],
  );
  assert.deepEqual(
    exclusionProblems(repoRoot, exclusions, readMetadata(repoRoot)),
    [],
  );
  assert.deepEqual(cargoArgs(exclusions), [
    "--exclude",
    "src-tauri",
    "--exclude",
    "e2e-app",
    "--exclude",
    "platform-macos",
    "--exclude",
    "secrets-keychain",
  ]);
  assert.equal(
    nextestFilter(exclusions),
    "not ((package(xtask) & (test(/^network_sandbox_tests::/) | test(/^nested_sandbox_tests::/))))",
  );
});

test("a stale or open exclusion list fails its verification", () => {
  const exclusions = readExclusions(repoRoot);
  const metadata = readMetadata(repoRoot);
  const withDependent = {
    ...metadata,
    packages: metadata.packages.map((member) =>
      member.name === "domain"
        ? {
            ...member,
            dependencies: [...member.dependencies, { name: "platform-macos" }],
          }
        : member,
    ),
  };
  assert.deepEqual(exclusionProblems(repoRoot, exclusions, withDependent), [
    "domain: 제외된 platform-macos에 의존하므로 함께 제외해야 한다",
  ]);
  assert.deepEqual(
    exclusionProblems(
      repoRoot,
      {
        crates: [
          ...exclusions.crates,
          { name: "ghost", reason: "없음" },
          exclusions.crates[0],
        ],
        tests: [
          ...exclusions.tests,
          { package: "xtask", module: "ghost_tests", reason: "없음" },
          { package: "src-tauri", module: "x", reason: "없음" },
        ],
      },
      metadata,
    ),
    [
      "src-tauri: 두 번 있다",
      "ghost: 작업 공간 crate가 아니다",
      "xtask::ghost_tests: 그런 모듈 선언이 없다",
      "src-tauri: crate 전체가 제외되어 테스트 제외가 필요 없다",
    ],
  );
  assert.equal(
    exclusionProblems(
      repoRoot,
      { crates: [{ name: "x" }], tests: [] },
      metadata,
    ).length,
    1,
  );
  assert.equal(nextestFilter({ crates: [], tests: [] }), "all()");
});

test("the Linux network block unshares the network as root and drops back to the caller", () => {
  const [program, args] = offlineCommand(
    { uid: "1001", gid: "1001", home: "/home/runner", path: "/usr/bin" },
    ["just", "linux-test"],
  );
  assert.equal(program, "sudo");
  assert.deepEqual(args, [
    "unshare",
    "--net",
    "bash",
    "-c",
    INNER_SCRIPT,
    "offline",
    "1001",
    "1001",
    "/home/runner",
    "/usr/bin",
    "just",
    "linux-test",
  ]);
  assert.match(
    INNER_SCRIPT,
    /^ip link set lo up && exec setpriv --reuid="\$1" --regid="\$2"/u,
  );
});

test("tool versions are pinned exactly and match the harness pins and this machine", () => {
  const tools = readTools(repoRoot);
  assert.equal(
    installActionSpec(tools),
    "just@1.59.0,cargo-nextest@0.9.151,cargo-deny@0.20.2,cargo-llvm-cov@0.9.1",
  );
  assert.deepEqual(tools.cargoInstall, { "cargo-mutants": "27.1.0" });
  const lefthookPin = /^version = "([^"]+)"$/mu.exec(
    readFileSync(join(repoRoot, "harness/git-hooks/lefthook.toml"), "utf8"),
  )[1];
  assert.equal(tools.lefthook.version, lefthookPin);
  ["Linux-ARM64", "Linux-X64", "macOS-ARM64"].forEach((key) => {
    const asset = lefthookAsset(tools, key);
    assert.match(
      asset.url,
      new RegExp(`/v${lefthookPin}/lefthook_${lefthookPin}_`, "u"),
    );
    assert.match(asset.sha256, /^[0-9a-f]{64}$/u);
  });
  assert.ok(lefthookAsset(tools, "Windows-X64").problem !== undefined);
  const versions = [
    ["just", ["--version"], `just ${tools.installAction.tools.just}`],
    [
      "cargo",
      ["nextest", "--version"],
      `cargo-nextest ${tools.installAction.tools["cargo-nextest"]}`,
    ],
    [
      "cargo",
      ["deny", "--version"],
      `cargo-deny ${tools.installAction.tools["cargo-deny"]}`,
    ],
    [
      "cargo",
      ["llvm-cov", "--version"],
      `cargo-llvm-cov ${tools.installAction.tools["cargo-llvm-cov"]}`,
    ],
    [
      "cargo",
      ["mutants", "--version"],
      `cargo-mutants ${tools.cargoInstall["cargo-mutants"]}`,
    ],
    ["lefthook", ["version"], lefthookPin],
  ];
  versions.forEach(([program, args, expected]) => {
    const finished = spawnSync(program, args, {
      cwd: repoRoot,
      encoding: "utf8",
    });
    assert.ok(
      finished.stdout.trim().startsWith(expected),
      `${program} ${args.join(" ")}: ${finished.stdout.trim()}`,
    );
  });
});

test("the prefetch list comes from the crates the Rust lint examples use offline", () => {
  const tools = readTools(repoRoot);
  const dependencies = prefetchDependencies(
    readFileSync(join(repoRoot, tools.prefetch.source), "utf8"),
  );
  assert.deepEqual(dependencies, [
    { name: "rust_decimal", version: "1.43.0" },
    { name: "num-traits", version: "0.2.19" },
  ]);
  assert.match(
    prefetchManifest(dependencies),
    /\nrust_decimal = \{ version = "=1\.43\.0", default-features = false \}\n/u,
  );
});

const pushTo = (ref, before, sha) => ({
  name: "push",
  ref,
  before,
  sha,
  prBase: "",
  prHead: "",
});

const A = "a".repeat(40);
const B = "b".repeat(40);
const C = "c".repeat(40);

const fakeRepository = ({ exists = true, ancestor = true, tip = C } = {}) => ({
  exists: () => exists,
  isAncestor: () => ancestor,
  remoteTip: () => (tip === "none" ? undefined : tip),
});

test("the base is the previous master tip, origin/master for branches and the target for PRs", () => {
  assert.deepEqual(
    rangeFor(pushTo("refs/heads/master", A, B), fakeRepository()),
    { base: A, head: B, notes: [`master 푸시: 이전 끝 ${A} 기준`] },
  );
  const forced = rangeFor(
    pushTo("refs/heads/master", A, B),
    fakeRepository({ ancestor: false }),
  );
  assert.equal(forced.base, A);
  assert.match(forced.notes[1], /^강제 푸시/u);
  assert.match(
    rangeFor(
      pushTo("refs/heads/master", A, B),
      fakeRepository({ exists: false }),
    ).problem,
    /가져오지 못했다/u,
  );
  assert.match(
    rangeFor(pushTo("refs/heads/master", ZERO, B), fakeRepository()).problem,
    /새로 만들어졌다/u,
  );
  assert.deepEqual(
    rangeFor(pushTo("refs/heads/h-16", ZERO, B), fakeRepository()),
    {
      base: C,
      head: B,
      notes: [`refs/heads/h-16 푸시: origin/master 끝 ${C} 기준 (새 브랜치)`],
    },
  );
  assert.ok(
    rangeFor(pushTo("refs/heads/h-16", A, B), fakeRepository({ tip: "none" }))
      .problem !== undefined,
  );
  assert.deepEqual(
    rangeFor(
      {
        name: "pull_request",
        ref: "",
        before: "",
        sha: C,
        prBase: A,
        prHead: B,
      },
      fakeRepository(),
    ),
    { base: A, head: B, notes: [`PR: 대상 브랜치 끝 ${A} 기준`] },
  );
  assert.ok(
    rangeFor({ ...pushTo("", A, B), name: "schedule" }, fakeRepository())
      .problem !== undefined,
  );
  assert.equal(changedRecently(1000, 1000 + 86400), "yes");
  assert.equal(changedRecently(1000, 1001 + 86400), "no");
});

test("the workflows run Linux on every push, macOS on master pushes and nightly, and notify on failure", () => {
  const ci = workflow("ci.yml");
  const nightly = workflow("nightly.yml");
  assert.deepEqual(ci.on, {
    push: { branches: ["**"] },
    pull_request: { branches: ["master"] },
  });
  assert.equal(ci.jobs.linux.if, undefined);
  assert.equal(ci.jobs.signatures.if, undefined);
  assert.equal(
    ci.jobs.macos.if,
    "github.event_name == 'push' && github.ref == 'refs/heads/master'",
  );
  assert.ok(runs(ci.jobs.linux).includes("just linux-check"));
  assert.ok(
    runs(ci.jobs.linux).includes(
      "node harness/ci/linux/offline.mjs just linux-test",
    ),
  );
  assert.ok(runs(ci.jobs.macos).includes("lefthook install"));
  assert.ok(runs(ci.jobs.macos).includes('just base="$BASE" check'));
  assert.ok(
    runs(ci.jobs.signatures).includes(
      'node harness/ci/signatures/from-base.mjs "$GITHUB_WORKSPACE" "$BASE" "$HEAD"',
    ),
  );
  assert.deepEqual(nightly.on, { schedule: [{ cron: "0 18 * * *" }] });
  assert.equal(nightly.jobs.macos.if, "needs.changes.outputs.changed == 'yes'");
  assert.ok(runs(nightly.jobs.macos).includes("just base=HEAD check"));
  assert.ok(
    runs(nightly.jobs.linux).includes(
      "node harness/ci/linux/offline.mjs just mutants-all",
    ),
  );
  assert.ok(runs(nightly.jobs.linux).includes("just advisory-db-fetch"));
  [
    ci.jobs.linux,
    ci.jobs.macos,
    nightly.jobs.linux,
    nightly.jobs.macos,
  ].forEach((job) => {
    const steps = runs(job);
    assert.ok(
      steps.indexOf("just advisory-db-fetch") <
        steps.findIndex(
          (step) =>
            /^(just|node harness\/ci\/linux)/u.test(step) &&
            step !== "just advisory-db-fetch",
        ),
    );
  });
  [ci, nightly].forEach((file) => {
    assert.match(file.jobs.notify.if, /^failure\(\)/u);
    assert.deepEqual(file.jobs.notify.permissions, { issues: "write" });
  });
  [ci, nightly].forEach((file) =>
    Object.values(file.jobs)
      .flatMap((job) => job.steps)
      .filter(
        (step) =>
          step.name === "cargo registry cache" ||
          step.name === "cargo-mutants cache",
      )
      .forEach((step) =>
        assert.doesNotMatch(
          step.with.path,
          /target|node_modules|registry\/src/u,
        ),
      ),
  );
});

test("signatures run from the base commit and say so when the base has no verifier", () => {
  const made = createRepository();
  try {
    const head = commit(made.directory, "next");
    const finished = spawnSync(
      "node",
      ["harness/ci/signatures/from-base.mjs", made.directory, made.base, head],
      { cwd: repoRoot, encoding: "utf8" },
    );
    assert.equal(finished.status, 0);
    assert.equal(
      finished.stdout,
      `서명 검사 비활성: 기준 커밋 ${made.base}에 ${VERIFIER}가 없다\n`,
    );
    assert.equal(runFromBase(made.directory, "f".repeat(40), head), 1);
  } finally {
    removeRepository(made.directory);
  }
});
