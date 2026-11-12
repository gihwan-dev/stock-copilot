import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  DEFINITIONS,
  decideBash,
} from "../../../.claude/hooks/lib/bash-guard.mjs";
import { LOCK } from "../../../.claude/hooks/lib/project.mjs";
import {
  applyVariant,
  createFixture,
  dataVariants,
  placeholders,
  projectFor,
  readJson,
  realRules,
  roleInput,
  ruleSet,
  runHook,
} from "./fixture.mjs";

const cases = readJson("harness/claude-hooks/bash-cases.json");
const fixture = createFixture();
const fill = placeholders(fixture);

const inputFor = (item) => ({
  hook_event_name: "PreToolUse",
  tool_name: "Bash",
  tool_input: {
    command: fill(item.command),
    ...(item.sandbox === true ? { dangerouslyDisableSandbox: true } : {}),
  },
  cwd: fill(item.cwd ?? "{root}"),
  ...roleInput(item.role),
});

const outcome = (item, rules = realRules, definitions = DEFINITIONS) => {
  const project = projectFor(fixture, {
    lock: item.lock === "on" ? LOCK.on : LOCK.off,
    rules,
  });
  const verdict = decideBash(inputFor(item), project, definitions);
  return { decision: verdict.decision, rules: ruleSet(verdict) };
};

const matches = (item, rules, definitions) => {
  const actual = outcome(item, rules, definitions);
  return (
    actual.decision === item.decision &&
    JSON.stringify(actual.rules) === JSON.stringify(item.rules)
  );
};

test("every bash case gets its expected decision and rules", () => {
  const wrong = cases
    .map((item) => ({ item, actual: outcome(item) }))
    .filter(
      ({ item, actual }) =>
        actual.decision !== item.decision ||
        JSON.stringify(actual.rules) !== JSON.stringify(item.rules),
    )
    .map(
      ({ item, actual }) =>
        `${JSON.stringify(item.command)} [${item.role ?? "main"}, lock ${item.lock ?? "off"}]: expected ${item.decision} ${item.rules.join(",")}, got ${actual.decision} ${actual.rules.join(",")}`,
    );
  assert.deepEqual(wrong, []);
});

test("known gaps are recorded as allowed cases with a reason", () => {
  const gaps = cases.filter((item) => item.gap !== undefined);
  assert.ok(gaps.length >= 5);
  gaps.forEach((item) => assert.equal(item.decision, "allow", item.command));
});

test("the brief's evasion variants each have a denying case", () => {
  const commands = new Set(
    cases
      .filter((item) => item.decision === "deny")
      .map((item) => item.command),
  );
  [
    "git   commit    -n   -m x",
    '"git" \'commit\' "-n" -m x',
    "X=1 git commit -n",
    "env X=1 git commit -n",
    'bash -lc "git commit --no-verify"',
    "sh -c 'git commit -n'",
    "true; git commit -n",
    "true && git commit -n",
    "echo x | git commit -n -F -",
    "(git commit -n)",
    "echo `git commit -n`",
    "echo $(git commit -n)",
    "eval 'git commit -n'",
    "true\ngit commit -n",
    "/usr/bin/git commit -n",
    "alias g='git commit -n'; g -m x",
    "command git commit -n",
    "echo x | xargs git commit -n",
    "bash <<'EOF'\ngit commit -n\nEOF",
    "git -C /tmp commit -n",
  ].forEach((command) => assert.ok(commands.has(command), command));
});

test("removing any rules.json element outside the main role patterns breaks some bash case", () => {
  const surviving = dataVariants(realRules)
    .filter((variant) => variant.path.join(".") !== "paths.mainPatterns")
    .filter((variant) => {
      const rules = applyVariant(realRules, variant);
      return cases.every((item) => matches(item, rules, DEFINITIONS));
    })
    .map((variant) => variant.name);
  assert.deepEqual(surviving, []);
});

const withoutEntry = (definitions, key, name) => {
  const table = definitions[key];
  return {
    ...definitions,
    [key]: Array.isArray(table)
      ? table.filter((entry) => (entry.id ?? entry) !== name)
      : Object.fromEntries(
          Object.entries(table).filter(([other]) => other !== name),
        ),
  };
};

test("removing any check, expander, wrapper, writer or reserved word breaks some bash case", () => {
  const names = Object.entries(DEFINITIONS).flatMap(([key, table]) =>
    (Array.isArray(table)
      ? table.map((entry) => entry.id ?? entry)
      : Object.keys(table)
    ).map((name) => [key, name]),
  );
  const surviving = names
    .filter(([key, name]) => {
      const definitions = withoutEntry(DEFINITIONS, key, name);
      return cases.every((item) => matches(item, realRules, definitions));
    })
    .map(([key, name]) => `${key}.${name}`);
  assert.deepEqual(surviving, []);
});

test("guard-bash.mjs answers Claude Code's PreToolUse JSON on stdin", () => {
  const denied = runHook("guard-bash.mjs", {
    hook_event_name: "PreToolUse",
    tool_name: "Bash",
    tool_input: { command: "git commit --no-verify -m x" },
    cwd: fixture.root,
  });
  assert.equal(denied.status, 0, denied.stderr);
  assert.equal(denied.output.hookSpecificOutput.hookEventName, "PreToolUse");
  assert.equal(denied.output.hookSpecificOutput.permissionDecision, "deny");
  assert.match(
    denied.output.hookSpecificOutput.permissionDecisionReason,
    /git-no-verify[\s\S]*멈춘 뒤 이유를 보고하라/u,
  );
  const allowed = runHook("guard-bash.mjs", {
    hook_event_name: "PreToolUse",
    tool_name: "Bash",
    tool_input: { command: "git status" },
    cwd: fixture.root,
  });
  assert.equal(allowed.status, 0, allowed.stderr);
  assert.equal(allowed.stdout, "");
  const pushing = runHook("guard-bash.mjs", {
    hook_event_name: "PreToolUse",
    tool_name: "Bash",
    tool_input: { command: "git push origin h-15" },
    cwd: fixture.root,
    agent_type: "implementer",
    agent_id: "a1",
  });
  assert.equal(pushing.output.hookSpecificOutput.permissionDecision, "deny");
});

test("guard-bash.mjs fails on unreadable input so onFailure blocks the call", () => {
  const broken = runHook("guard-bash.mjs", "not json");
  assert.notEqual(broken.status, 0);
  assert.equal(broken.stdout, "");
});
