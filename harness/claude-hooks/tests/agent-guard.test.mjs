import { strict as assert } from "node:assert";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import {
  cpSync,
  existsSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { agentFindings } from "../../../.claude/hooks/lib/agent-guard.mjs";
import {
  BRIEF_PROBLEM,
  parseBrief,
} from "../../../.claude/hooks/lib/brief.mjs";
import { lineDigest } from "../../../.claude/hooks/lib/normalize.mjs";
import {
  applyVariant,
  createFixture,
  dataVariants,
  readJson,
  repoRoot,
  roleInput,
} from "./fixture.mjs";

const rules = readJson("harness/claude-hooks/agent-rules.json");
const cases = readJson("harness/claude-hooks/agent-cases.json");

const TIP = "a".repeat(40);
const PASSED = "b".repeat(40);
const OTHER = "c".repeat(40);
const COMMITS = new Map([
  [TIP, TIP],
  ["aaaaaaa", TIP],
  [PASSED, PASSED],
  [OTHER, OTHER],
]);

const facts = {
  resolveCommit: (text) => COMMITS.get(text),
  baseTip: TIP,
  passedTargets: new Map([[PASSED, "a0"]]),
  reportDigests: new Set([
    lineDigest("- 가격 정렬은 BTreeMap으로 바꿨고 경계값 테스트를 더했다"),
  ]),
  worktreeForBranch: (branch) =>
    new Map([
      ["h-25", "/wt/h-25"],
      ["topic", "/wt/topic"],
    ]).get(branch),
};

const inputFor = (item) => ({
  hook_event_name: "PreToolUse",
  tool_name: item.tool,
  tool_input: item.input,
  ...roleInput(item.role),
});

const outcome = (item, chosen = rules) => {
  const findings = agentFindings(inputFor(item), chosen, facts);
  return {
    decision: findings.length === 0 ? "allow" : "deny",
    rules: [...new Set(findings.map((finding) => finding.rule))].sort(),
  };
};

const mismatches = (chosen) =>
  cases
    .map((item) => ({ item, actual: outcome(item, chosen) }))
    .filter(
      ({ item, actual }) =>
        actual.decision !== item.decision ||
        JSON.stringify(actual.rules) !== JSON.stringify(item.rules),
    )
    .map(
      ({ item, actual }) =>
        `${item.name}: expected ${item.decision} ${item.rules.join(",")}, got ${actual.decision} ${actual.rules.join(",")}`,
    );

test("every agent case gets its expected decision and rules", () => {
  assert.deepEqual(mismatches(rules), []);
});

test("removing any agent rule element breaks some agent case", () => {
  const survivors = dataVariants(rules)
    .filter((variant) => !variant.name.startsWith("reportFirstLines"))
    .filter((variant) => mismatches(applyVariant(rules, variant)).length === 0)
    .map((variant) => variant.name);
  assert.deepEqual(survivors, []);
});

test("emptying either report first-line marker breaks some agent case", () => {
  ["done", "blocked"].forEach((key) => {
    const changed = {
      ...rules,
      reportFirstLines: { ...rules.reportFirstLines, [key]: "\u0000" },
    };
    assert.ok(mismatches(changed).length > 0, key);
  });
});

test("the agent rules allow exactly the three documented subagents and arguments", () => {
  assert.deepEqual(rules.subagentTypes, [
    "implementer",
    "adversarial-reviewer",
    "review-critic",
  ]);
  assert.deepEqual(rules.allowedArguments, [
    "description",
    "prompt",
    "subagent_type",
  ]);
  assert.deepEqual(rules.forbiddenTools, ["SendMessage"]);
  assert.equal(rules.baseRef, "refs/heads/master");
  assert.equal(rules.maxBlocks, 3);
});

test("brief parsing reads the base commit and branch the workflow template writes", () => {
  assert.deepEqual(parseBrief(`- 기준 커밋: ${TIP}`), {
    commit: TIP,
    branch: undefined,
  });
  assert.deepEqual(parseBrief(`- 기준 커밋: \`abc1234\`, 브랜치 \`h-25\``), {
    commit: "abc1234",
    branch: "h-25",
  });
  assert.deepEqual(parseBrief(`* 기준 커밋 : abc1234\n- 브랜치: h-25`), {
    commit: "abc1234",
    branch: "h-25",
  });
  assert.equal(parseBrief("기준커밋 abc1234").problem, BRIEF_PROBLEM.noBase);
  assert.equal(parseBrief(undefined).problem, BRIEF_PROBLEM.noBase);
  assert.equal(
    parseBrief(`- 기준 커밋: abc123`).problem,
    BRIEF_PROBLEM.badBase,
  );
  assert.equal(parseBrief(`- 기준 커밋: HEAD`).problem, BRIEF_PROBLEM.badBase);
});

const hookFixture = () => {
  const fixture = createFixture();
  cpSync(join(repoRoot, ".claude/hooks"), join(fixture.root, ".claude/hooks"), {
    recursive: true,
  });
  [
    "harness/claude-hooks/agent-rules.json",
    "harness/claude-hooks/rules.json",
  ].forEach((path) => cpSync(join(repoRoot, path), join(fixture.root, path)));
  const tip = execFileSync(
    "git",
    ["-C", fixture.root, "rev-parse", "refs/heads/master"],
    { encoding: "utf8" },
  ).trim();
  return { ...fixture, tip };
};

const runGuard = (input, state, root = repoRoot) => {
  const finished = spawnSync(
    process.execPath,
    [join(root, ".claude/hooks/guard-agent.mjs")],
    {
      input: JSON.stringify(input),
      encoding: "utf8",
      env: {
        ...process.env,
        CLAUDE_PROJECT_DIR: root,
        STOCK_COPILOT_HARNESS_STATE: state,
      },
    },
  );
  const text = finished.stdout.trim();
  return {
    status: finished.status,
    output: text === "" ? undefined : JSON.parse(text),
    stderr: finished.stderr,
  };
};

test("guard-agent.mjs denies like Claude Code expects and queues allowed implementers", () => {
  const state = realpathSync(mkdtempSync(join(tmpdir(), "agent-guard-")));
  const denied = runGuard(
    {
      session_id: "s1",
      tool_use_id: "t1",
      tool_name: "Agent",
      tool_input: { subagent_type: "general-purpose", prompt: "x" },
    },
    state,
  );
  assert.equal(denied.status, 0, denied.stderr);
  assert.equal(denied.output.hookSpecificOutput.permissionDecision, "deny");
  assert.match(
    denied.output.hookSpecificOutput.permissionDecisionReason,
    /\[agent-type\]/u,
  );
  const hooked = hookFixture();
  const prompt = `# 브리프\n- 기준 커밋: ${hooked.tip}, 브랜치 \`side\``;
  const allowed = runGuard(
    {
      session_id: "s1",
      tool_use_id: "t2",
      tool_name: "Agent",
      tool_input: {
        subagent_type: "implementer",
        description: "d",
        prompt,
      },
    },
    state,
    hooked.root,
  );
  assert.equal(allowed.status, 0, allowed.stderr);
  assert.equal(allowed.output, undefined);
  const queued = JSON.parse(
    readFileSync(join(state, "records", "pending-s1-t2.json"), "utf8"),
  );
  assert.deepEqual(
    [queued.sessionId, queued.toolUseId, queued.prompt],
    ["s1", "t2", prompt],
  );
  const reviewer = runGuard(
    {
      session_id: "s1",
      tool_use_id: "t3",
      tool_name: "Agent",
      tool_input: {
        subagent_type: "review-critic",
        description: "d",
        prompt: "리뷰 브리프 전문",
      },
    },
    state,
  );
  assert.equal(reviewer.output, undefined);
  assert.equal(existsSync(join(state, "records", "pending-s1-t3.json")), false);
  const broken = spawnSync(
    process.execPath,
    [join(repoRoot, ".claude/hooks/guard-agent.mjs")],
    { input: "{", encoding: "utf8" },
  );
  assert.notEqual(broken.status, 0);
});

test("two implementer launches queued at the same time both stay queued", async () => {
  const state = realpathSync(mkdtempSync(join(tmpdir(), "agent-guard-race-")));
  const hooked = hookFixture();
  const prompt = `# 브리프\n- 기준 커밋: ${hooked.tip}, 브랜치 \`side\``;
  const launch = (id) =>
    new Promise((resolve) => {
      const child = spawn(
        process.execPath,
        [join(hooked.root, ".claude/hooks/guard-agent.mjs")],
        {
          env: {
            ...process.env,
            CLAUDE_PROJECT_DIR: hooked.root,
            STOCK_COPILOT_HARNESS_STATE: state,
          },
        },
      );
      child.on("close", resolve);
      child.stdin.end(
        JSON.stringify({
          session_id: "s2",
          tool_use_id: id,
          tool_name: "Agent",
          tool_input: {
            subagent_type: "implementer",
            description: "d",
            prompt,
          },
        }),
      );
    });
  await Promise.all([launch("t1"), launch("t2")]);
  assert.deepEqual(readdirSync(join(state, "records")).sort(), [
    "pending-s2-t1.json",
    "pending-s2-t2.json",
  ]);
});
