import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { ZERO, changedRecently, rangeFor } from "../ci-range.mjs";
import { existingIssue, issueBody } from "../notify.mjs";
import {
  cargoArgs,
  exclusionProblems,
  nextestFilter,
  readExclusions,
  readMetadata,
} from "../linux/exclusions.mjs";
import { INNER_SCRIPT, offlineCommand } from "../linux/offline.mjs";
import { VERIFIER, parseArguments } from "../signatures/from-base.mjs";
import {
  fetchedOwnerKeys,
  ownerKeysFromResponse,
  ownerKeysRequest,
  ownerKeysUrl,
} from "../signatures/owner-keys.mjs";
import {
  installActionSpec,
  lefthookAsset,
  prefetchDependencies,
  prefetchManifest,
  readTools,
} from "../tools.mjs";
import {
  childEnvironment,
  commit,
  createRepository,
  gitIn,
  removeRepository,
  repoRoot,
  run,
  runBaseStep,
  scratch,
  workflowRun,
  write,
} from "./signature-repo.mjs";

const VERIFIER_FILES = [
  "harness/ci/signatures/verify.mjs",
  "harness/ci/signatures/manifests.mjs",
  "harness/ci/signatures/owner-keys.mjs",
  "harness/ci/signatures/owner.txt",
  "harness/claude-hooks/rules.json",
  "harness/ci/signatures/lock-point.txt",
  ".claude/hooks/lib/allowlist.mjs",
  "harness/agent-writable.txt",
  "harness/allowed_signers",
];

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
      env: childEnvironment(),
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
  assert.equal(ci.jobs.signatures, undefined);
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
  const signatures = workflow("signatures.yml");
  assert.deepEqual(signatures.on, ci.on);
  assert.deepEqual(signatures.jobs.signatures.permissions, {
    contents: "read",
    actions: "read",
  });
  const signatureSteps = signatures.jobs.signatures.steps;
  assert.deepEqual(signatureSteps[3].with, {
    ref: "${{ steps.base.outputs.tools }}",
    path: "base-tools",
    "fetch-depth": 1,
    "persist-credentials": false,
  });
  assert.deepEqual(
    signatureSteps.slice(4).map((step) => [step.if, step.run]),
    [
      [
        "hashFiles('base-tools/harness/ci/signatures/from-base.mjs') != ''",
        'node base-tools/harness/ci/signatures/from-base.mjs "$GITHUB_WORKSPACE" "$HEAD" --range-start "$RANGE_START" --base-kind "$BASE_KIND"',
      ],
      [
        "hashFiles('base-tools/harness/ci/signatures/from-base.mjs') == ''",
        "echo '서명 검사 비활성: 기준 checkout에 harness/ci/signatures/from-base.mjs가 없다'",
      ],
    ],
  );
  assert.deepEqual(signatureSteps[4].env, {
    GH_TOKEN: "${{ github.token }}",
    HEAD: "${{ github.event.pull_request.head.sha || github.sha }}",
    RANGE_START: "${{ steps.base.outputs.base }}",
    BASE_KIND: "${{ steps.base.outputs.kind }}",
  });
  assert.equal(
    signatures.jobs.notify.steps[0].with.ref,
    "${{ needs.signatures.outputs.tools }}",
  );
  [ci, nightly, signatures].forEach((file) =>
    assert.match(
      file.jobs.notify.steps[2].run,
      /^node base-tools\/harness\/ci\/notify\.mjs$/u,
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
  [ci, nightly, workflow("signatures.yml")].forEach((file) => {
    assert.match(file.jobs.notify.if, /^failure\(\)/u);
    assert.deepEqual(file.jobs.notify.permissions, { issues: "write" });
  });
  [ci, nightly].forEach((file) =>
    Object.values(file.jobs)
      .flatMap((job) => job.steps)
      .forEach((step) =>
        assert.doesNotMatch(String(step.uses ?? ""), /^actions\/cache/u),
      ),
  );
});

const fromBaseCheckout = (made, revision, keysFile) => {
  const tools = join(scratch("signature-tools-"), "tools");
  run(made.directory, "git", ["clone", "--quiet", made.directory, tools]);
  run(tools, "git", ["checkout", "--quiet", "--detach", revision]);
  return {
    tools,
    finished: spawnSync(
      "node",
      [
        join(tools, "harness/ci/signatures/from-base.mjs"),
        made.directory,
        gitIn(made.directory, ["rev-parse", "HEAD"]),
        "--range-start",
        revision,
        "--base-kind",
        "unverified",
        "--owner-keys",
        keysFile,
      ],
      { cwd: repoRoot, encoding: "utf8", env: childEnvironment() },
    ),
  };
};

test("signatures run the verifier of a base checkout and fail when it ends without its summary", () => {
  const made = createRepository();
  const cleanup = [made.directory];
  try {
    [...VERIFIER_FILES, "harness/ci/signatures/from-base.mjs"].forEach((path) =>
      write(made.directory, path, readFileSync(join(repoRoot, path), "utf8")),
    );
    const base = commit(made.directory, "verifier");
    write(made.directory, "owner-keys.txt", "");
    const keysFile = join(made.directory, "owner-keys.txt");
    write(made.directory, VERIFIER, "");
    const head = commit(made.directory, "drop the verifier body");
    const good = fromBaseCheckout(made, base, keysFile);
    cleanup.push(join(good.tools, ".."));
    assert.equal(good.finished.status, 0, good.finished.stderr);
    assert.match(good.finished.stdout, /^서명 검사 비활성: 잠금 시점 없음/mu);
    assert.match(good.finished.stdout, new RegExp(`기준 커밋 ${base}`, "u"));
    assert.match(good.finished.stdout, /^서명 검사: 위반 0건$/mu);
    const silent = fromBaseCheckout(made, head, keysFile);
    cleanup.push(join(silent.tools, ".."));
    assert.equal(silent.finished.status, 1);
    assert.match(silent.finished.stderr, /요약 줄 없이 끝났다/u);
  } finally {
    cleanup.forEach(removeRepository);
  }
});

test("the base runner takes the target, and owner keys come from GitHub with the job token", () => {
  assert.deepEqual(
    parseArguments([
      "r",
      "h",
      "--range-start",
      "s",
      "--base-kind",
      "verified",
      "--owner-keys",
      "f",
    ]),
    {
      repo: join(process.cwd(), "r"),
      head: "h",
      rangeStart: "s",
      passed: ["--base-kind", "verified", "--owner-keys", "f"],
    },
  );
  [
    [],
    ["r"],
    ["r", "h"],
    ["r", "h", "--range-start", "s"],
    ["r", "h", "--base-kind", "verified"],
    ["r", "h", "--range-start", "s", "--base-kind"],
    [
      "r",
      "h",
      "--range-start",
      "s",
      "--base-kind",
      "verified",
      "--owner",
      "me",
    ],
    [
      "r",
      "h",
      "--range-start",
      "s",
      "--range-start",
      "t",
      "--base-kind",
      "verified",
    ],
  ].forEach((args) =>
    assert.ok(parseArguments(args).problem !== undefined, args.join(" ")),
  );
  assert.equal(
    ownerKeysUrl("gihwan-dev"),
    "https://api.github.com/users/gihwan-dev/ssh_signing_keys",
  );
  const [program, args] = ownerKeysRequest("gihwan-dev", "t0ken");
  assert.equal(program, "curl");
  assert.ok(args.includes("Authorization: Bearer t0ken"));
  assert.equal(
    args.at(-1),
    "https://api.github.com/users/gihwan-dev/ssh_signing_keys",
  );
  assert.ok(
    !ownerKeysRequest("gihwan-dev", "")[1].some((arg) =>
      arg.startsWith("Authorization"),
    ),
  );
  assert.ok(fetchedOwnerKeys("", "t")().problem !== undefined);
  assert.ok(fetchedOwnerKeys("a/b", "t")().problem !== undefined);
  assert.equal(
    readFileSync(join(repoRoot, "harness/ci/signatures/owner.txt"), "utf8"),
    "gihwan-dev\n",
  );
  assert.deepEqual(
    ownerKeysFromResponse(
      JSON.stringify([
        { id: 1, key: "ssh-ed25519 AAAA1 title", title: "a" },
        { id: 2, key: "ecdsa-sha2-nistp256 AAAA2" },
        { id: 3 },
      ]),
    ),
    { keys: ["ssh-ed25519 AAAA1", "ecdsa-sha2-nistp256 AAAA2"] },
  );
  assert.ok(ownerKeysFromResponse("{}").problem !== undefined);
  assert.ok(ownerKeysFromResponse("<html>").problem !== undefined);
});

test("failure notices reuse the open issue with exactly the same title", () => {
  const listing = JSON.stringify([
    { number: 3, title: "CI 실패: master" },
    { number: 4, title: "CI 실패: 밤 작업" },
  ]);
  assert.equal(existingIssue(listing, "CI 실패: 밤 작업"), "4");
  assert.equal(existingIssue(listing, "CI 실패"), undefined);
  assert.equal(existingIssue("not json", "CI 실패: master"), undefined);
  assert.equal(
    issueBody("https://example.invalid/run/1", "abc"),
    "실패한 실행: https://example.invalid/run/1\n커밋: abc\n",
  );
});

test("the signature base is the newest master commit a successful signature run verified", () => {
  const made = createRepository();
  try {
    const verified = commit(made.directory, "verified");
    const failed = commit(made.directory, "failed push");
    const head = commit(made.directory, "next push");
    const master = {
      EVENT_NAME: "push",
      REF: "refs/heads/master",
      BEFORE: failed,
      HEAD: head,
    };
    assert.deepEqual(
      runBaseStep(made, master, ["f".repeat(40), verified, made.base]),
      {
        status: 0,
        output: { tools: verified, base: verified, kind: "verified" },
      },
    );
    assert.deepEqual(runBaseStep(made, master, []), {
      status: 0,
      output: { tools: failed, base: failed, kind: "unverified" },
    });
    assert.equal(runBaseStep(made, master, ["f".repeat(40)]).status, 1);
    gitIn(made.directory, [
      "update-ref",
      "refs/remotes/origin/master",
      verified,
    ]);
    gitIn(made.directory, ["checkout", "--quiet", "-b", "feature", made.base]);
    const branch = commit(made.directory, "branch work");
    const push = {
      EVENT_NAME: "push",
      REF: "refs/heads/feature",
      BEFORE: "0".repeat(40),
      HEAD: branch,
    };
    assert.deepEqual(runBaseStep(made, push, [head]), {
      status: 0,
      output: { tools: head, base: made.base, kind: "verified" },
    });
    assert.deepEqual(runBaseStep(made, push, []), {
      status: 0,
      output: { tools: verified, base: made.base, kind: "unverified" },
    });
  } finally {
    removeRepository(made.directory);
  }
});

test("the workflow passes the base kind from its base step to the verifier, so an unverified base with a lock fails", () => {
  const made = createRepository();
  const steps = workflow("signatures.yml").jobs.signatures.steps;
  const runner = steps.find(
    (step) => step.name === "signatures from the base checkout",
  );
  try {
    [...VERIFIER_FILES, "harness/ci/signatures/from-base.mjs"].forEach((path) =>
      write(made.directory, path, readFileSync(join(repoRoot, path), "utf8")),
    );
    write(
      made.directory,
      "harness/ci/signatures/lock-point.txt",
      `${made.base}\n`,
    );
    write(made.directory, ".gitignore", "base-tools/\n");
    const locked = commit(made.directory, "verifier and lock");
    const head = commit(made.directory, "next");
    const outcome = (runs) => {
      const based = runBaseStep(
        made,
        {
          EVENT_NAME: "push",
          REF: "refs/heads/master",
          BEFORE: locked,
          HEAD: head,
        },
        runs,
      );
      removeRepository(join(made.directory, "base-tools"));
      run(made.directory, "git", [
        "clone",
        "--quiet",
        made.directory,
        "base-tools",
      ]);
      run(join(made.directory, "base-tools"), "git", [
        "checkout",
        "--quiet",
        "--detach",
        based.output.tools,
      ]);
      return workflowRun(made, runner, {
        HEAD: head,
        RANGE_START: based.output.base,
        BASE_KIND: based.output.kind,
      });
    };
    const unverified = outcome([]);
    assert.equal(unverified.status, 1, unverified.stdout);
    assert.match(
      unverified.stdout,
      /서명 검사가 통과한 master 커밋이 아닌데 잠금 시점이 기록돼 있다/u,
    );
    const verified = outcome([locked]);
    assert.equal(verified.status, 0, verified.stdout + verified.stderr);
    assert.match(verified.stdout, /^서명 검사: 위반 0건$/mu);
    const missingKind = workflowRun(made, runner, {
      HEAD: head,
      RANGE_START: locked,
      BASE_KIND: "",
    });
    assert.equal(missingKind.status, 1);
    assert.match(missingKind.stderr, /--base-kind verified\|unverified/u);
  } finally {
    removeRepository(made.directory);
  }
});

test("a branch grown from before the lock reads trust data from the verified master commit and fails", () => {
  const made = createRepository();
  const steps = workflow("signatures.yml").jobs.signatures.steps;
  const runner = steps.find(
    (step) => step.name === "signatures from the base checkout",
  );
  try {
    write(made.directory, ".gitignore", "base-tools/\n");
    const old = commit(made.directory, "old, before the verifier");
    [...VERIFIER_FILES, "harness/ci/signatures/from-base.mjs"].forEach((path) =>
      write(made.directory, path, readFileSync(join(repoRoot, path), "utf8")),
    );
    write(made.directory, "harness/ci/signatures/lock-point.txt", `${old}\n`);
    const verified = commit(made.directory, "verifier and lock");
    gitIn(made.directory, [
      "update-ref",
      "refs/remotes/origin/master",
      verified,
    ]);
    gitIn(made.directory, ["checkout", "--quiet", "-b", "attack", made.base]);
    write(made.directory, "justfile", "check:\n    true\n# changed\n");
    const branch = commit(
      made.directory,
      "unsigned protected change from an old fork",
    );
    const based = runBaseStep(
      made,
      {
        EVENT_NAME: "push",
        REF: "refs/heads/attack",
        BEFORE: "0".repeat(40),
        HEAD: branch,
      },
      [verified],
    );
    assert.deepEqual(based.output, {
      tools: verified,
      base: made.base,
      kind: "verified",
    });
    run(made.directory, "git", [
      "clone",
      "--quiet",
      made.directory,
      "base-tools",
    ]);
    run(join(made.directory, "base-tools"), "git", [
      "checkout",
      "--quiet",
      "--detach",
      verified,
    ]);
    const finished = workflowRun(made, runner, {
      HEAD: branch,
      RANGE_START: based.output.base,
      BASE_KIND: based.output.kind,
    });
    assert.equal(finished.status, 1, finished.stdout);
    assert.match(finished.stdout, /의 자손이 아니다/u);
  } finally {
    removeRepository(made.directory);
  }
});
