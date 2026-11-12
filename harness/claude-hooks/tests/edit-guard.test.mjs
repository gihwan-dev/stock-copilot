import { strict as assert } from "node:assert";
import { join } from "node:path";
import { test } from "node:test";
import { parseAllowlist } from "../../../.claude/hooks/lib/allowlist.mjs";
import { decideEdit } from "../../../.claude/hooks/lib/edit-guard.mjs";
import { LOCK } from "../../../.claude/hooks/lib/project.mjs";
import {
  applyVariant,
  createFixture,
  dataVariants,
  placeholders,
  projectFor,
  readJson,
  realAllowlist,
  realRules,
  repoRoot,
  roleInput,
  ruleSet,
  runHook,
  withoutIndex,
} from "./fixture.mjs";
import { readFileSync } from "node:fs";

const cases = readJson("harness/claude-hooks/edit-cases.json");
const fixture = createFixture();
const fill = placeholders(fixture);

const inputFor = (item) => {
  const tool = item.tool ?? "Write";
  const path = item.path === null ? undefined : fill(item.path);
  const key = tool === "NotebookEdit" ? "notebook_path" : "file_path";
  return {
    hook_event_name: "PreToolUse",
    tool_name: tool,
    tool_input: path === undefined ? {} : { [key]: path, content: "x\n" },
    cwd: fixture.root,
    ...roleInput(item.role),
  };
};

const outcome = (item, overrides = {}) => {
  const verdict = decideEdit(
    inputFor(item),
    projectFor(fixture, {
      ...overrides,
      lock: item.lock === "on" ? LOCK.on : LOCK.off,
    }),
  );
  return { decision: verdict.decision, rules: ruleSet(verdict) };
};

const matches = (item, overrides) => {
  const actual = outcome(item, overrides);
  return (
    actual.decision === item.decision &&
    JSON.stringify(actual.rules) === JSON.stringify(item.rules)
  );
};

test("every edit case gets its expected decision and rules", () => {
  const wrong = cases
    .map((item) => ({ item, actual: outcome(item) }))
    .filter(
      ({ item, actual }) =>
        actual.decision !== item.decision ||
        JSON.stringify(actual.rules) !== JSON.stringify(item.rules),
    )
    .map(
      ({ item, actual }) =>
        `${item.path} [${item.role ?? "main"}, lock ${item.lock ?? "off"}]: expected ${item.decision} ${item.rules.join(",")}, got ${actual.decision} ${actual.rules.join(",")}`,
    );
  assert.deepEqual(wrong, []);
});

test("removing any agent-writable.txt line breaks some edit case", () => {
  const lines = readFileSync(
    join(repoRoot, "harness/agent-writable.txt"),
    "utf8",
  )
    .slice(0, -1)
    .split("\n");
  const surviving = lines.filter((line, index) => {
    const allowlist = parseAllowlist(
      `${withoutIndex(lines, index).join("\n")}\n`,
    );
    return cases.every((item) => matches(item, { allowlist }));
  });
  assert.deepEqual(surviving, []);
});

test("removing any rules.json path element breaks some edit case", () => {
  const surviving = dataVariants(realRules.paths, ["paths"])
    .filter((variant) => {
      const rules = applyVariant(realRules, variant);
      return cases.every((item) => matches(item, { rules }));
    })
    .map((variant) => variant.name);
  assert.deepEqual(surviving, []);
});

test("an unreadable allowlist refuses every edit", () => {
  const verdict = decideEdit(
    inputFor({ path: "{root}/docs/notes.md" }),
    projectFor(fixture, { allowlist: { problem: "broken" } }),
  );
  assert.equal(verdict.decision, "deny");
  assert.deepEqual(ruleSet(verdict), ["allowlist-unreadable"]);
});

test("guard-edit.mjs answers Claude Code's PreToolUse JSON with the real data files", () => {
  const write = (path, extra = {}) =>
    runHook("guard-edit.mjs", {
      hook_event_name: "PreToolUse",
      tool_name: "Write",
      tool_input: { file_path: path, content: "x\n" },
      cwd: repoRoot,
      ...extra,
    });
  const implementer = { agent_type: "implementer", agent_id: "a1" };
  const allowed = write(
    join(repoRoot, "crates/domain/src/lib.rs"),
    implementer,
  );
  assert.equal(allowed.status, 0, allowed.stderr);
  assert.equal(allowed.stdout, "");
  const warned = write(join(repoRoot, "justfile"), implementer);
  assert.equal(warned.status, 0, warned.stderr);
  assert.equal(warned.output.hookSpecificOutput.hookEventName, "PreToolUse");
  assert.equal(warned.output.hookSpecificOutput.permissionDecision, undefined);
  assert.match(
    warned.output.systemMessage,
    /protected-path[\s\S]*잠금 스위치/u,
  );
  const reviewer = write(join(repoRoot, "docs/notes.md"), {
    agent_type: "adversarial-reviewer",
    agent_id: "a2",
  });
  assert.equal(reviewer.output.hookSpecificOutput.permissionDecision, "deny");
  const outside = write("/etc/hosts", implementer);
  assert.equal(outside.output.hookSpecificOutput.permissionDecision, "deny");
  assert.match(
    outside.output.hookSpecificOutput.permissionDecisionReason,
    /작업 사본 밖[\s\S]*멈춘 뒤 이유를 보고하라/u,
  );
  const main = write(join(repoRoot, "crates/domain/src/lib.rs"));
  assert.equal(main.output.hookSpecificOutput.permissionDecision, "deny");
});

test("guard-edit.mjs fails on unreadable input so onFailure blocks the call", () => {
  const broken = runHook("guard-edit.mjs", "{");
  assert.notEqual(broken.status, 0);
  assert.equal(broken.stdout, "");
});

test("the real allowlist parses", () => {
  assert.equal(realAllowlist.problem, undefined);
});
