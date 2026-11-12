import { strict as assert } from "node:assert";
import { execFileSync, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { parseAllowlist } from "../../../.claude/hooks/lib/allowlist.mjs";
import { agentResult } from "../../../.claude/hooks/lib/agent-result.mjs";
import { digestOf } from "../../../.claude/hooks/lib/brief.mjs";
import {
  FRESH_STEPS,
  checkPlaces,
  checkProfile,
  runHarnessCheck,
} from "../../../.claude/hooks/lib/check-sandbox.mjs";
import {
  REPORT,
  decideImplementerStop,
  protectedChanges,
  reportKind,
} from "../../../.claude/hooks/lib/implementer-stop.mjs";
import { gitEnvironment, LOCK } from "../../../.claude/hooks/lib/project.mjs";
import {
  decideReviewerStop,
  snapshotOf,
} from "../../../.claude/hooks/lib/reviewer-guard.mjs";
import {
  STATUS,
  claimRecord,
  readRecord,
  stateDirectory,
  writeRecord,
} from "../../../.claude/hooks/lib/state.mjs";
import {
  choosePending,
  implementerStart,
} from "../../../.claude/hooks/lib/subagent-start.mjs";
import {
  contentSnapshot,
  treeProblems,
} from "../../../.claude/hooks/lib/tree-hash.mjs";
import {
  createFixture,
  readJson,
  realAllowlist,
  repoRoot,
} from "./fixture.mjs";

const rules = readJson("harness/claude-hooks/agent-rules.json");
const BASE = "a".repeat(40);
const HEAD = "e".repeat(40);
const PROMPT = `# 브리프 H-9\n- 기준 커밋: ${BASE}, 브랜치 \`h-9\``;

const startedRecord = (blocks = 0) => ({
  kind: "implementer",
  agentId: "a1",
  sessionId: "s1",
  base: BASE,
  branch: "h-9",
  promptDigest: digestOf(PROMPT),
  blocks,
  status: STATUS.unchecked,
});

const fakeFacts = (overrides = {}) => {
  const calls = [];
  return {
    calls,
    lock: LOCK.off,
    allowlist: realAllowlist,
    locateWorktree: (branch) => (branch === "h-9" ? "/wt/h-9" : undefined),
    headOf: () => HEAD,
    isAncestor: () => true,
    changedSince: () => ["crates/domain/src/lib.rs"],
    existingAdrs: () => ["docs/adr/0001-product-decisions.md"],
    briefText: () => PROMPT,
    treeProblems: () => [],
    runCheck: (worktree, base) => {
      calls.push([worktree, base]);
      return { status: 0, output: "check: ok" };
    },
    now: () => "2026-11-12T00:00:00.000Z",
    ...overrides,
  };
};

const failing = { runCheck: () => ({ status: 1, output: "FAIL some::test" }) };

const stop = (message, record, overrides) => {
  const facts = fakeFacts(overrides);
  const result = decideImplementerStop(
    { agent_id: "a1", session_id: "s1", last_assistant_message: message },
    record,
    facts,
    rules,
  );
  return { ...result, calls: facts.calls };
};

test("report first lines are exactly 완료 or 막힘: with a reason", () => {
  assert.equal(reportKind("완료\n- 커밋 1", rules), REPORT.done);
  assert.equal(reportKind("막힘: 테스트가 틀렸다", rules), REPORT.blocked);
  assert.equal(reportKind("막힘:", rules), REPORT.invalid);
  assert.equal(reportKind("완료했다", rules), REPORT.invalid);
  assert.equal(reportKind("Done", rules), REPORT.invalid);
  assert.equal(reportKind(undefined, rules), REPORT.invalid);
});

test("a passing check with a valid report lets the implementer stop as passed", () => {
  const result = stop("완료", startedRecord());
  assert.equal(result.output, undefined);
  assert.equal(result.record.status, STATUS.passed);
  assert.equal(result.record.head, HEAD);
  assert.equal(result.record.worktree, "/wt/h-9");
  assert.deepEqual(result.calls, [["/wt/h-9", BASE]]);
});

test("a failing check blocks three times and then allows only a 막힘: report", () => {
  const first = stop("완료", startedRecord(0), failing);
  assert.equal(first.output.decision, "block");
  assert.match(first.output.reason, /실패 \(1\/3\)[\s\S]*FAIL some::test/u);
  assert.equal(first.record.blocks, 1);
  assert.equal(first.record.status, STATUS.failing);
  const third = stop("완료", startedRecord(2), failing);
  assert.equal(third.output.decision, "block");
  assert.match(third.output.reason, /마지막 차단이다/u);
  assert.equal(third.record.blocks, 3);
  const early = stop("막힘: 못 고친다", startedRecord(1), failing);
  assert.equal(early.output.decision, "block");
  const stillDone = stop("완료", startedRecord(3), failing);
  assert.equal(stillDone.output.decision, "block");
  assert.equal(stillDone.record.blocks, 4);
  const blocked = stop("막힘: 기존 테스트가 틀렸다", startedRecord(3), failing);
  assert.equal(blocked.output.decision, undefined);
  assert.match(blocked.output.systemMessage, /막힘/u);
  assert.equal(blocked.record.status, STATUS.blocked);
});

test("an invalid report first line blocks even when the check passes", () => {
  const result = stop("작업을 마쳤다", startedRecord());
  assert.equal(result.output.decision, "block");
  assert.match(result.output.reason, /완료.*막힘/u);
  assert.equal(result.record.status, STATUS.passed);
  assert.equal(result.record.blocks, 1);
});

test("a base that is not an ancestor of HEAD fails without running the check", () => {
  const result = stop("완료", startedRecord(), { isAncestor: () => false });
  assert.equal(result.output.decision, "block");
  assert.match(result.output.reason, /조상이 아니다/u);
  assert.equal(result.record.status, STATUS.failing);
  assert.deepEqual(result.calls, []);
});

test("a missing worktree or a changed brief fails without running the check", () => {
  const lost = stop("완료", { ...startedRecord(), branch: "other" });
  assert.match(lost.output.reason, /작업 사본을 찾지 못했다/u);
  assert.deepEqual(lost.calls, []);
  const swapped = stop("완료", startedRecord(), {
    briefText: () => "다른 브리프",
  });
  assert.match(swapped.output.reason, /브리프와 다르다/u);
  assert.deepEqual(swapped.calls, []);
});

test("protected changes only warn while the lock is off and fail once it is on", () => {
  const changes = {
    changedSince: () => [
      "crates/domain/src/lib.rs",
      "justfile",
      "docs/adr/0001-product-decisions.md",
      "docs/adr/0099-new.md",
    ],
  };
  const off = stop("완료", startedRecord(), changes);
  assert.equal(off.record.status, STATUS.passed);
  assert.deepEqual(off.record.protectedChanges, [
    "docs/adr/0001-product-decisions.md",
    "justfile",
  ]);
  assert.match(off.output.systemMessage, /justfile/u);
  const on = stop("완료", startedRecord(), { ...changes, lock: LOCK.on });
  assert.equal(on.output.decision, "block");
  assert.match(on.output.reason, /보호 경로 변경: justfile/u);
  assert.deepEqual(on.calls, []);
  assert.deepEqual(
    protectedChanges(["a.txt"], parseAllowlist("a.txt\n"), []),
    [],
  );
  assert.deepEqual(protectedChanges(["a.txt"], { problem: "형식 오류" }, []), [
    "a.txt",
  ]);
});

test("a stop without a recorded base is left unchecked and not blocked", () => {
  const missing = stop("완료", undefined);
  assert.equal(missing.record.status, STATUS.unchecked);
  assert.equal(missing.output.decision, undefined);
  assert.deepEqual(missing.calls, []);
  const baseless = stop("완료", {
    kind: "implementer",
    agentId: "a1",
    status: STATUS.unchecked,
    reason: "브리프에 기준 커밋이 없다",
  });
  assert.match(baseless.output.systemMessage, /기준 커밋이 없다/u);
});

test("implementer start records the base only when it is the tip or a passed target", () => {
  const facts = {
    resolveCommit: (text) => (text === "aaaaaaa" ? BASE : undefined),
    baseTip: BASE,
    passedTargets: new Map(),
    worktreeForBranch: (branch) => (branch === "h-9" ? "/wt/h-9" : undefined),
    now: () => "t",
  };
  const started = implementerStart(
    { agent_id: "a1", session_id: "s1" },
    "- 기준 커밋: aaaaaaa, 브랜치 `h-9`",
    facts,
    rules,
  );
  assert.equal(started.base, BASE);
  assert.equal(started.baseSource, "base-branch");
  assert.equal(started.branch, "h-9");
  assert.equal(started.worktree, "/wt/h-9");
  assert.equal(started.prompt, undefined);
  assert.equal(started.status, STATUS.unchecked);
  const rejected = implementerStart(
    { agent_id: "a1" },
    "- 기준 커밋: bbbbbbb, 브랜치 `h-9`",
    facts,
    rules,
  );
  assert.equal(rejected.base, undefined);
  assert.match(rejected.reason, /풀 수 없다/u);
  const unread = implementerStart({ agent_id: "a1" }, undefined, facts, rules);
  const original = implementerStart(
    { agent_id: "a1" },
    "- 기준 커밋: aaaaaaa, 브랜치 `master`",
    facts,
    rules,
  );
  assert.equal(original.base, undefined);
  assert.match(original.reason, /기준 브랜치/u);
  assert.equal(unread.base, undefined);
});

test("pending launches are chosen by digest, tool use or age and claimed once", () => {
  const entries = [
    { sessionId: "s", digest: "d2", toolUseId: "t2", prompt: "p2", at: "2" },
    { sessionId: "s", digest: "d1", toolUseId: "t1", prompt: "p1", at: "1" },
  ];
  assert.equal(choosePending(entries, "d2").prompt, "p2");
  assert.equal(choosePending(entries, undefined, "t2").prompt, "p2");
  assert.equal(choosePending(entries).prompt, "p1");
  assert.equal(choosePending(entries, "zz"), undefined);
  assert.equal(choosePending([]), undefined);
  const state = realpathSync(mkdtempSync(join(tmpdir(), "agent-claim-")));
  writeRecord(state, "pending", "s-t1", entries[1]);
  assert.equal(claimRecord(state, "pending", "s-t1", "a1").prompt, "p1");
  assert.equal(claimRecord(state, "pending", "s-t1", "a2"), undefined);
  assert.equal(readRecord(state, "pending", "s-t1"), undefined);
});

test("state lives outside the worktree and only takes absolute overrides", () => {
  assert.equal(
    stateDirectory({}, "/Users/x"),
    "/Users/x/.local/state/stock-copilot-harness",
  );
  assert.equal(
    stateDirectory({ STOCK_COPILOT_HARNESS_STATE: "/s" }, "/Users/x"),
    "/s",
  );
  assert.equal(
    stateDirectory({ STOCK_COPILOT_HARNESS_STATE: "rel" }, "/Users/x"),
    "/Users/x/.local/state/stock-copilot-harness",
  );
  const state = realpathSync(mkdtempSync(join(tmpdir(), "agent-state-")));
  writeRecord(state, "implementer", "a1", { status: STATUS.passed });
  assert.deepEqual(readRecord(state, "implementer", "a1"), {
    status: STATUS.passed,
  });
  assert.equal(readRecord(state, "implementer", "../x"), undefined);
  assert.throws(() => writeRecord(state, "implementer", "../x", {}));
});

test("the post-agent context reports the record and treats a missing one as unchecked", () => {
  const record = {
    ...startedRecord(),
    status: STATUS.passed,
    head: HEAD,
    worktree: "/wt/h-9",
  };
  const read = (kind, id) =>
    kind === "implementer" && id === "a1" ? record : undefined;
  const input = (id, status = "completed") => ({
    tool_input: { subagent_type: "implementer" },
    tool_response: {
      status,
      agentId: id,
      resolvedModel: "claude-opus-5-5",
      content: [
        {
          type: "text",
          text: "완료\n- 가격 정렬을 바꾸고 테스트를 더했다고 적은 줄",
        },
      ],
    },
  });
  const passed = agentResult(input("a1"), read, () => HEAD);
  const text = passed.output.hookSpecificOutput.additionalContext;
  assert.match(text, /상태=passed/u);
  assert.match(text, /모델=claude-opus-5-5,/u);
  assert.equal(passed.record.resolvedModel, "claude-opus-5-5");
  assert.equal(JSON.stringify(passed.record).includes("가격 정렬"), false);
  const moved = agentResult(input("a1"), read, () => "f".repeat(40));
  assert.match(
    moved.output.hookSpecificOutput.additionalContext,
    /상태=unchecked[\s\S]*HEAD가/u,
  );
  const missing = agentResult(input("zz"), read, () => HEAD);
  assert.match(
    missing.output.hookSpecificOutput.additionalContext,
    /상태=unchecked/u,
  );
  const background = agentResult(
    input("a1", "async_launched"),
    read,
    () => HEAD,
  );
  assert.match(
    background.output.hookSpecificOutput.additionalContext,
    /unchecked/u,
  );
  const otherModel = agentResult(
    {
      ...input("a1"),
      tool_response: {
        ...input("a1").tool_response,
        resolvedModel: "claude-haiku-5-5",
      },
    },
    read,
    () => HEAD,
  );
  assert.match(
    otherModel.output.hookSpecificOutput.additionalContext,
    /정의의 claude-opus-5-5가 아니다/u,
  );
});

test("reviewer stops are blocked while any worktree's content differs from the start", () => {
  const fixture = createFixture();
  const worktrees = [fixture.root, fixture.worktree];
  const facts = { contentSnapshot };
  writeFileSync(join(fixture.root, "docs/notes.md"), "dirty before\n");
  const before = snapshotOf(worktrees, fixture.root, facts);
  const record = {
    kind: "reviewer",
    agentId: "r1",
    snapshot: before,
    blocks: 0,
  };
  const input = { agent_id: "r1", agent_type: "adversarial-reviewer" };
  const now = (list = worktrees) => snapshotOf(list, fixture.root, facts);
  const decide = (snapshot, given = record) =>
    decideReviewerStop(input, given, snapshot, rules, "t");
  mkdirSync(join(fixture.root, ".claude/worktrees/agent-r1"), {
    recursive: true,
  });
  writeFileSync(join(fixture.root, ".claude/worktrees/agent-r1/x"), "x\n");
  assert.equal(decide(now()).output, undefined);
  assert.equal(decide(now()).record.status, "clean");
  writeFileSync(join(fixture.root, "docs/notes.md"), "dirty and more\n");
  const further = decide(now());
  assert.equal(further.output.decision, "block");
  assert.match(further.output.reason, /내용이 바뀐 파일 docs\/notes\.md/u);
  writeFileSync(join(fixture.root, "docs/notes.md"), "dirty before\n");
  writeFileSync(join(fixture.worktree, "docs/escape.md"), "x\n");
  const added = decide(now());
  assert.match(added.output.reason, /새 파일 docs\/escape\.md/u);
  assert.equal(added.record.blocks, 1);
  rmSync(join(fixture.worktree, "docs/escape.md"));
  execFileSync(
    "git",
    ["-C", fixture.root, "mv", "docs/harness.md", "docs/moved.md"],
    {
      env: gitEnvironment(),
    },
  );
  const renamed = decide(now());
  assert.match(renamed.output.reason, /사라진 파일 docs\/harness\.md/u);
  assert.match(renamed.output.reason, /새 파일 docs\/moved\.md/u);
  assert.match(renamed.output.reason, /인덱스가 바뀌었다/u);
  execFileSync(
    "git",
    ["-C", fixture.root, "mv", "docs/moved.md", "docs/harness.md"],
    {
      env: gitEnvironment(),
    },
  );
  assert.equal(decide(now()).output, undefined);
  const created = decide(now([...worktrees, fixture.outside]));
  assert.match(created.output.reason, /새 작업 사본|상태를 읽지 못했다/u);
  const broken = decide([...now(), { path: "/x", failed: "git이 실패했다" }]);
  assert.equal(broken.output.decision, "block");
  const given = decide(now([fixture.root]), {
    ...record,
    blocks: rules.maxBlocks,
  });
  assert.equal(given.output.decision, undefined);
  assert.match(given.output.systemMessage, /바꾼 채 끝났다/u);
  assert.equal(given.record.status, "contaminated");
  const unknown = decideReviewerStop(input, undefined, [], rules, "t");
  assert.match(unknown.output.systemMessage, /스냅숏이 없어/u);
});

test("the stop hook refuses a worktree whose files differ from its commit", () => {
  const fixture = createFixture();
  const root = fixture.worktree;
  const git = (...args) =>
    execFileSync(
      "git",
      [
        "-c",
        "user.name=t",
        "-c",
        "user.email=t@e.invalid",
        "-c",
        "commit.gpgsign=false",
        "-C",
        root,
        ...args,
      ],
      { env: gitEnvironment(), encoding: "utf8" },
    );
  writeFileSync(join(root, ".gitignore"), "/target/\n");
  git("add", ".gitignore");
  git("commit", "-q", "-m", "ignore");
  assert.deepEqual(treeProblems(root), []);
  mkdirSync(join(root, "target"), { recursive: true });
  writeFileSync(join(root, "target/build.o"), "x\n");
  assert.deepEqual(treeProblems(root), []);
  writeFileSync(join(root, "crates/domain/src/lib.rs"), "weakened\n");
  git("commit", "-q", "-am", "weaken");
  writeFileSync(join(root, "crates/domain/src/lib.rs"), "fixture\n");
  assert.deepEqual(treeProblems(root), [
    "스테이징하지 않은 변경: crates/domain/src/lib.rs",
  ]);
  git("add", "crates/domain/src/lib.rs");
  assert.deepEqual(treeProblems(root), [
    "스테이징된 변경: crates/domain/src/lib.rs",
  ]);
  git("reset", "-q", "--hard");
  writeFileSync(join(root, "docs/new.md"), "x\n");
  assert.deepEqual(treeProblems(root), ["추적하지 않는 파일: docs/new.md"]);
  rmSync(join(root, "docs/new.md"));
  git("update-index", "--assume-unchanged", "docs/notes.md");
  writeFileSync(join(root, "docs/notes.md"), "hidden\n");
  assert.deepEqual(treeProblems(root), [
    "assume-unchanged/skip-worktree 표시: docs/notes.md",
    "스테이징하지 않은 변경: docs/notes.md",
  ]);
  const result = stop("완료", startedRecord(), {
    treeProblems: () => ["스테이징하지 않은 변경: x"],
  });
  assert.equal(result.output.decision, "block");
  assert.match(result.output.reason, /작업 트리가 커밋과 다르다/u);
  assert.deepEqual(result.calls, []);
  assert.match(treeProblems(fixture.outside)[0], /git이 실패했다/u);
});

const sandboxed = (justfile) => {
  const base = realpathSync(mkdtempSync(join(tmpdir(), "check-sandbox-")));
  const worktree = join(base, "worktree");
  const project = join(base, "project");
  const state = join(base, "state");
  const home = join(base, "home");
  [worktree, join(project, ".claude"), state, join(home, ".ssh")].forEach(
    (path) => mkdirSync(path, { recursive: true }),
  );
  writeFileSync(join(home, ".ssh/id_probe"), "secret\n");
  writeFileSync(join(state, "record.json"), "{}\n");
  writeFileSync(
    join(worktree, "justfile"),
    typeof justfile === "function"
      ? justfile({ state, project, home })
      : justfile,
  );
  const places = checkPlaces({
    worktree,
    common: join(project, ".git"),
    projectRoot: project,
    state,
    home,
  });
  return { base, worktree, project, state, home, places };
};

const NESTING =
  spawnSync("/usr/bin/sandbox-exec", [
    "-p",
    '(version 1)(allow default)(deny file-write* (literal "/private/var/empty/stock-copilot-nesting"))',
    "/usr/bin/true",
  ]).status === 0;

const sandboxTest = (name, body) =>
  test(name, () => {
    if (NESTING) {
      body();
      return;
    }
    assert.equal(process.env.SANDBOX_RUNTIME, "1");
  });

test("the hook check profile blocks the network, secrets, state and hook writes", () => {
  const { places, state, home, project } = sandboxed(
    'base := ""\ncheck:\n    true\n',
  );
  const profile = checkProfile(places);
  assert.match(profile, /\(deny network-outbound\)/u);
  assert.ok(
    profile.includes(`(subpath ${JSON.stringify(join(home, ".ssh"))})`),
  );
  assert.ok(
    profile.includes(`(subpath ${JSON.stringify(join(home, ".claude"))})`),
  );
  assert.ok(profile.includes(`(subpath ${JSON.stringify(state)})`));
  assert.ok(
    profile.includes(`(subpath ${JSON.stringify(join(project, ".claude"))})`),
  );
  assert.equal(profile.includes("unix-socket))"), false);
  assert.deepEqual(FRESH_STEPS.at(-1).slice(-2), [
    "--offline",
    "--ignore-scripts",
  ]);
  assert.ok(NESTING || process.env.SANDBOX_RUNTIME === "1");
});

const touchStep = (name) => ["/usr/bin/touch", name];

sandboxTest(
  "the hook check runs the offline fresh steps first and then just base=<SHA> check",
  () => {
    assert.deepEqual(FRESH_STEPS, [
      ["cargo", "clean", "--locked", "--offline", "--workspace"],
      ["cargo", "llvm-cov", "clean", "--locked", "--offline", "--workspace"],
      [
        "pnpm",
        "install",
        "--frozen-lockfile",
        "--force",
        "--offline",
        "--ignore-scripts",
      ],
    ]);
    const { worktree, places } = sandboxed(
      'set shell := ["bash", "-c"]\nbase := ""\ncheck:\n    test -f fresh.marker && printf "checked %s" "{{base}}" > checked.txt\n',
    );
    const result = runHarnessCheck(places, worktree, BASE, [
      touchStep("fresh.marker"),
    ]);
    assert.equal(result.status, 0, result.output);
    assert.equal(
      readFileSync(join(worktree, "checked.txt"), "utf8"),
      `checked ${BASE}`,
    );
    const broken = sandboxed('base := ""\ncheck:\n    touch checked.txt\n');
    const refused = runHarnessCheck(broken.places, broken.worktree, BASE, [
      ["/usr/bin/false"],
    ]);
    assert.notEqual(refused.status, 0);
    assert.match(refused.output, /이미 받아 둔 의존성/u);
    assert.equal(existsSync(join(broken.worktree, "checked.txt")), false);
  },
);

sandboxTest(
  "code run by the hook check cannot write the state or hooks, read secrets or reach the network",
  () => {
    const { worktree, state, project, places } = sandboxed(
      ({ state, project, home }) =>
        `set shell := ["bash", "-c"]\nbase := ""\ncheck:\n    touch inside.txt\n    echo '{}' > ${join(state, "forged.json")} || true\n    echo x > ${join(project, ".claude", "hook.mjs")} || true\n    cat ${join(home, ".ssh/id_probe")} > leaked.txt || rm -f leaked.txt\n    cat ${join(state, "record.json")} > state-read.txt || rm -f state-read.txt\n    /usr/bin/nc -z -G 3 1.1.1.1 443 && touch online.txt || true\n    /usr/bin/curl -s -m 5 -o /dev/null https://registry.npmjs.org/ && touch registry.txt || true\n    printf '%s' "\${SSH_AUTH_SOCK:-none}" > agent.txt\n`,
    );
    const result = runHarnessCheck(places, worktree, BASE, []);
    assert.equal(result.status, 0, result.output);
    assert.equal(existsSync(join(worktree, "inside.txt")), true);
    assert.equal(existsSync(join(state, "forged.json")), false);
    assert.equal(existsSync(join(project, ".claude", "hook.mjs")), false);
    assert.equal(existsSync(join(worktree, "leaked.txt")), false);
    assert.equal(existsSync(join(worktree, "state-read.txt")), false);
    assert.equal(existsSync(join(worktree, "online.txt")), false);
    assert.equal(existsSync(join(worktree, "registry.txt")), false);
    assert.equal(readFileSync(join(worktree, "agent.txt"), "utf8"), "none");
  },
);

sandboxTest(
  "a postinstall script is skipped by the hook's offline install and cannot read secrets",
  () => {
    const { worktree, places, home } = sandboxed(
      'base := ""\ncheck:\n    true\n',
    );
    writeFileSync(
      join(worktree, "package.json"),
      `${JSON.stringify({
        name: "probe",
        private: true,
        scripts: {
          postinstall: `cat ${join(home, ".ssh/id_probe")} > ran.txt`,
        },
      })}\n`,
    );
    const result = runHarnessCheck(places, worktree, BASE, [
      ["pnpm", "install", "--offline", "--ignore-scripts"],
    ]);
    assert.equal(result.status, 0, result.output);
    assert.equal(existsSync(join(worktree, "ran.txt")), false);
    const profile = checkProfile(places);
    assert.match(profile, /\(deny file-read\* [^)]*\.ssh/u);
    assert.match(profile, /\(deny network-outbound\)/u);
  },
);

sandboxTest(
  "with the lock on the hook check cannot write the fragment's protected paths",
  () => {
    const { worktree, places } = sandboxed(() => "");
    const locked = checkPlaces({
      worktree,
      common: join(worktree, ".git"),
      projectRoot: worktree,
      state: join(worktree, "state"),
      locked: ["./harness", "./*.json", "./justfile"],
    });
    writeFileSync(
      join(worktree, "justfile"),
      'set shell := ["bash", "-c"]\nbase := ""\ncheck:\n    mkdir -p target\n    mkdir -p harness || true\n    echo x > harness/x.txt || true\n    echo x > top.json || true\n    echo x > target/ok.txt\n',
    );
    const result = runHarnessCheck(locked, worktree, BASE, []);
    assert.equal(result.output.includes("justfile"), false, result.output);
    assert.equal(existsSync(join(worktree, "harness/x.txt")), false);
    assert.equal(existsSync(join(worktree, "top.json")), false);
    assert.equal(existsSync(join(worktree, "target/ok.txt")), true);
    assert.notEqual(places.denied.length, locked.denied.length);
  },
);

test("hook entry scripts record a start and leave an unrecorded stop unchecked", () => {
  const state = realpathSync(mkdtempSync(join(tmpdir(), "agent-entry-")));
  const run = (script, input) =>
    spawnSync(process.execPath, [join(repoRoot, ".claude/hooks", script)], {
      input: JSON.stringify(input),
      encoding: "utf8",
      env: {
        ...process.env,
        CLAUDE_PROJECT_DIR: repoRoot,
        STOCK_COPILOT_HARNESS_STATE: state,
      },
    });
  const tip = execFileSync(
    "git",
    ["-C", repoRoot, "rev-parse", "refs/heads/master"],
    { encoding: "utf8" },
  ).trim();
  writeRecord(state, "pending", "s9-t9", {
    sessionId: "s9",
    digest: "d",
    toolUseId: "t9",
    prompt: `- 기준 커밋: ${tip}, 브랜치 \`master\``,
    at: "1",
  });
  const started = run("subagent-start.mjs", {
    session_id: "s9",
    agent_id: "a9",
    agent_type: "implementer",
    hook_event_name: "SubagentStart",
    transcript_path: join(state, "missing.jsonl"),
  });
  assert.equal(started.status, 0, started.stderr);
  assert.match(
    JSON.parse(started.stdout).hookSpecificOutput.additionalContext,
    /기록하지 못했다[\s\S]*기준 브랜치/u,
  );
  const recorded = readRecord(state, "implementer", "a9");
  assert.equal(recorded.base, undefined);
  assert.equal(recorded.prompt, undefined);
  assert.equal(readRecord(state, "pending", "s9-t9"), undefined);
  const stopped = run("implementer-stop.mjs", {
    session_id: "s9",
    agent_id: "a8",
    agent_type: "implementer",
    hook_event_name: "SubagentStop",
    last_assistant_message: "완료",
  });
  assert.equal(stopped.status, 0, stopped.stderr);
  assert.match(JSON.parse(stopped.stdout).systemMessage, /unchecked/u);
  assert.equal(readRecord(state, "implementer", "a8").status, STATUS.unchecked);
  const result = run("agent-result.mjs", {
    hook_event_name: "PostToolUse",
    tool_name: "Agent",
    tool_input: { subagent_type: "implementer" },
    tool_response: { status: "completed", agentId: "a8", content: [] },
  });
  assert.equal(result.status, 0, result.stderr);
  assert.match(
    JSON.parse(result.stdout).hookSpecificOutput.additionalContext,
    /상태=unchecked/u,
  );
  [
    "subagent-start.mjs",
    "implementer-stop.mjs",
    "reviewer-stop.mjs",
    "agent-result.mjs",
  ].forEach((script) => {
    const broken = spawnSync(
      process.execPath,
      [join(repoRoot, ".claude/hooks", script)],
      { input: "{", encoding: "utf8" },
    );
    assert.notEqual(broken.status, 0, script);
  });
});
