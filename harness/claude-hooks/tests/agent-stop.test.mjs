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
  checkPlaces,
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
  readRecord,
  stateDirectory,
  writeRecord,
} from "../../../.claude/hooks/lib/state.mjs";
import {
  claimPending,
  implementerStart,
  withPending,
} from "../../../.claude/hooks/lib/subagent-start.mjs";
import { headOf, statusOf } from "../../../.claude/hooks/lib/git-facts.mjs";
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
  prompt: PROMPT,
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
    briefDigest: () => digestOf(PROMPT),
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
    briefDigest: () => digestOf("다른 브리프"),
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
    now: () => "t",
  };
  const started = implementerStart(
    { agent_id: "a1", session_id: "s1" },
    "- 기준 커밋: aaaaaaa, 브랜치 `h-9`",
    facts,
  );
  assert.equal(started.base, BASE);
  assert.equal(started.baseSource, "base-branch");
  assert.equal(started.branch, "h-9");
  assert.equal(started.status, STATUS.unchecked);
  const rejected = implementerStart(
    { agent_id: "a1" },
    "- 기준 커밋: bbbbbbb",
    facts,
  );
  assert.equal(rejected.base, undefined);
  assert.match(rejected.reason, /풀 수 없다/u);
  const unread = implementerStart({ agent_id: "a1" }, undefined, facts);
  assert.equal(unread.base, undefined);
});

test("pending launches are claimed by digest, tool use or age", () => {
  const queue = withPending(
    withPending(undefined, {
      type: "implementer",
      digest: "d1",
      toolUseId: "t1",
      prompt: "p1",
    }),
    { type: "implementer", digest: "d2", toolUseId: "t2", prompt: "p2" },
  );
  assert.equal(claimPending(queue, "implementer", "d2").prompt, "p2");
  assert.equal(
    claimPending(queue, "implementer", undefined, "t2").prompt,
    "p2",
  );
  const oldest = claimPending(queue, "implementer");
  assert.equal(oldest.prompt, "p1");
  assert.deepEqual(
    oldest.queue.entries.map((entry) => entry.prompt),
    ["p2"],
  );
  assert.equal(claimPending(queue, "review-critic").prompt, undefined);
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
  const passed = agentResult(input("a1"), read, rules, () => HEAD);
  const text = passed.output.hookSpecificOutput.additionalContext;
  assert.match(text, /상태=passed/u);
  assert.match(text, /모델=claude-opus-5-5,/u);
  assert.deepEqual(passed.record.reportLines, [
    "- 가격 정렬을 바꾸고 테스트를 더했다고 적은 줄",
  ]);
  const moved = agentResult(input("a1"), read, rules, () => "f".repeat(40));
  assert.match(
    moved.output.hookSpecificOutput.additionalContext,
    /상태=unchecked[\s\S]*HEAD가/u,
  );
  const missing = agentResult(input("zz"), read, rules, () => HEAD);
  assert.match(
    missing.output.hookSpecificOutput.additionalContext,
    /상태=unchecked/u,
  );
  const background = agentResult(
    input("a1", "async_launched"),
    read,
    rules,
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
    rules,
    () => HEAD,
  );
  assert.match(
    otherModel.output.hookSpecificOutput.additionalContext,
    /정의의 claude-opus-5-5가 아니다/u,
  );
});

test("reviewer stops are blocked while the original or a worktree differs from the start", () => {
  const fixture = createFixture();
  const worktrees = [fixture.root, fixture.worktree];
  const facts = { headOf, statusOf };
  const before = snapshotOf(worktrees, fixture.root, facts);
  const record = {
    kind: "reviewer",
    agentId: "r1",
    snapshot: before,
    blocks: 0,
  };
  const input = { agent_id: "r1", agent_type: "adversarial-reviewer" };
  mkdirSync(join(fixture.root, ".claude/worktrees/agent-r1"), {
    recursive: true,
  });
  writeFileSync(join(fixture.root, ".claude/worktrees/agent-r1/x"), "x\n");
  const isolated = decideReviewerStop(
    input,
    record,
    snapshotOf(worktrees, fixture.root, facts),
    rules,
    "t",
  );
  assert.equal(isolated.output, undefined);
  assert.equal(isolated.record.status, "clean");
  writeFileSync(join(fixture.root, "docs/notes.md"), "changed\n");
  writeFileSync(join(fixture.worktree, "docs/escape.md"), "x\n");
  const dirty = decideReviewerStop(
    input,
    record,
    snapshotOf(worktrees, fixture.root, facts),
    rules,
    "t",
  );
  assert.equal(dirty.output.decision, "block");
  assert.match(dirty.output.reason, /docs\/notes\.md[\s\S]*docs\/escape\.md/u);
  assert.equal(dirty.record.blocks, 1);
  const given = decideReviewerStop(
    input,
    { ...record, blocks: rules.maxBlocks },
    snapshotOf(worktrees, fixture.root, facts),
    rules,
    "t",
  );
  assert.equal(given.output.decision, undefined);
  assert.match(given.output.systemMessage, /바꾼 채 끝났다/u);
  assert.equal(given.record.status, "contaminated");
  execFileSync("git", ["-C", fixture.root, "checkout", "--", "docs/notes.md"], {
    env: gitEnvironment(),
  });
  rmSync(join(fixture.worktree, "docs/escape.md"));
  const reverted = decideReviewerStop(
    input,
    record,
    snapshotOf(worktrees, fixture.root, facts),
    rules,
    "t",
  );
  assert.equal(reverted.output, undefined);
  const unknown = decideReviewerStop(input, undefined, [], rules, "t");
  assert.match(unknown.output.systemMessage, /스냅숏이 없어/u);
});

const sandboxed = (justfile) => {
  const base = realpathSync(mkdtempSync(join(tmpdir(), "check-sandbox-")));
  const worktree = join(base, "worktree");
  const project = join(base, "project");
  const state = join(base, "state");
  [worktree, join(project, ".claude"), state].forEach((path) =>
    mkdirSync(path, { recursive: true }),
  );
  writeFileSync(
    join(worktree, "justfile"),
    typeof justfile === "function" ? justfile({ state, project }) : justfile,
  );
  const places = checkPlaces({
    worktree,
    common: join(project, ".git"),
    projectRoot: project,
    state,
  });
  return { base, worktree, project, state, places };
};

test("the hook check runs fresh-build first and then just base=<SHA> check", () => {
  const { worktree, places } = sandboxed(
    'set shell := ["bash", "-c"]\nfresh-build:\n    touch fresh.marker\n\nbase := ""\ncheck:\n    test -f fresh.marker && printf "checked %s" "{{base}}" > checked.txt\n',
  );
  const result = runHarnessCheck(places, worktree, BASE);
  assert.equal(result.status, 0, result.output);
  assert.equal(
    readFileSync(join(worktree, "checked.txt"), "utf8"),
    `checked ${BASE}`,
  );
  const broken = sandboxed(
    'fresh-build:\n    exit 3\n\nbase := ""\ncheck:\n    touch checked.txt\n',
  );
  const refused = runHarnessCheck(broken.places, broken.worktree, BASE);
  assert.notEqual(refused.status, 0);
  assert.equal(existsSync(join(broken.worktree, "checked.txt")), false);
});

test("code run by the hook check cannot write the state, the hooks or the network", () => {
  const { worktree, state, project, places } = sandboxed(
    ({ state, project }) =>
      `set shell := ["bash", "-c"]\nfresh-build:\n    true\n\nbase := ""\ncheck:\n    touch inside.txt\n    echo '{}' > ${join(state, "forged.json")} || true\n    echo x > ${join(project, ".claude", "hook.mjs")} || true\n    /usr/bin/nc -z -G 3 1.1.1.1 443 && touch online.txt || true\n`,
  );
  const result = runHarnessCheck(places, worktree, BASE);
  assert.equal(result.status, 0, result.output);
  assert.equal(existsSync(join(worktree, "inside.txt")), true);
  assert.equal(existsSync(join(state, "forged.json")), false);
  assert.equal(existsSync(join(project, ".claude", "hook.mjs")), false);
  assert.equal(existsSync(join(worktree, "online.txt")), false);
});

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
  writeRecord(state, "pending", "s9", {
    entries: [
      {
        type: "implementer",
        digest: "d",
        toolUseId: "t9",
        prompt: `- 기준 커밋: ${tip}, 브랜치 \`h-9\``,
      },
    ],
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
    new RegExp(tip, "u"),
  );
  assert.equal(readRecord(state, "implementer", "a9").base, tip);
  assert.deepEqual(readRecord(state, "pending", "s9").entries, []);
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
  const reviewer = run("subagent-start.mjs", {
    session_id: "s9",
    agent_id: "r9",
    agent_type: "review-critic",
    hook_event_name: "SubagentStart",
  });
  assert.equal(reviewer.status, 0, reviewer.stderr);
  const snapshot = readRecord(state, "reviewer", "r9").snapshot;
  assert.ok(snapshot.some((entry) => entry.path === repoRoot));
  const reviewed = run("reviewer-stop.mjs", {
    session_id: "s9",
    agent_id: "r9",
    agent_type: "review-critic",
    hook_event_name: "SubagentStop",
  });
  assert.equal(reviewed.status, 0, reviewed.stderr);
  assert.equal(readRecord(state, "reviewer", "r9").status, "clean");
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
