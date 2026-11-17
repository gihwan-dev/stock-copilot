import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { orchestrationFindings } from "../../../.claude/hooks/lib/orchestration.mjs";
const rules = JSON.parse(
  readFileSync(
    new URL("../../claude-hooks/agent-rules.json", import.meta.url),
    "utf8",
  ),
);
const base = "a".repeat(40);
const facts = {
  baseTip: base,
  resolveCommit: (value) => value,
  passedTargets: new Map(),
  reportDigests: new Set(),
  worktreeForBranch: () => "/target",
  headOf: () => "b".repeat(40),
  isAncestor: () => true,
};
const brief = `- 기준 커밋: ${base}, 브랜치 \`topic\``;
const verdict = (tool_input, launch) =>
  orchestrationFindings(
    {
      tool_name: launch
        ? "mcp__t3_code__t3_thread_launch"
        : "mcp__t3_code__delegate_task",
      tool_input,
    },
    rules,
    facts,
  );
const delegate = (model, role, options) => ({
  isolation: "worktree",
  target: { driverKind: "codex", model, options },
  role,
  task: brief + (role === "review" ? `\n- 대상 SHA: ${"b".repeat(40)}` : ""),
});
const launch = (model, role, options) => ({
  modelSelection: { provider: "codex", model, options },
  workspaceStrategy: {
    type: "worktree",
    branch: "topic",
    baseRef: base,
    startFromOrigin: false,
  },
  message: brief + (role === undefined ? "" : `\n- 역할: ${role}`),
});
for (const model of [
  "gpt-6.1-sol",
  "gpt-6.1-terra",
  "gpt-6-luna",
  "gpt-6.1-terra-extra",
  "gpt-6.1-sol-extra",
  "",
  undefined,
]) {
  for (const role of [
    "implementation",
    "research",
    "review",
    undefined,
    "general",
    "critic",
  ]) {
    test(`delegate exact model ${model} role ${role}`, () => {
      const accepted =
        (model === "gpt-6.1-sol" &&
          ["implementation", "research", "review", undefined].includes(role)) ||
        (model === "gpt-6.1-terra" && role === "implementation");
      assert.equal(
        verdict(
          delegate(model, role, {
            effort: model === "gpt-6.1-terra" ? "max" : "high",
          }),
        ).length,
        accepted ? 0 : 1,
      );
    });
    test(`launch exact model ${model} role ${role}`, () => {
      const accepted =
        model === "gpt-6.1-sol" ||
        (model === "gpt-6.1-terra" && role === "implementation");
      assert.equal(
        verdict(
          launch(model, role, {
            effort: model === "gpt-6.1-terra" ? "max" : "high",
          }),
          true,
        ).length,
        accepted ? 0 : 1,
      );
    });
  }
}
for (const model of ["gpt-6.1-sol", "gpt-6.1-terra"]) {
  for (const effort of ["high", "max", "low", "ultra"]) {
    test(`model effort ${model} ${effort}`, () => {
      const expected =
        effort === (model === "gpt-6.1-terra" ? "max" : "high") ? 0 : 1;
      assert.equal(
        verdict(delegate(model, "implementation", { effort })).length,
        expected,
      );
      assert.equal(
        verdict(launch(model, "implementation", { effort }), true).length,
        expected,
      );
    });
  }
  test(`model boundaries ${model}`, () => {
    for (const patch of [
      { isolation: "shared" },
      { runtimeMode: "full-access" },
    ])
      assert.equal(
        verdict({
          ...delegate(model, "implementation", {
            reasoningEffort: model === "gpt-6.1-terra" ? "max" : "high",
          }),
          ...patch,
        }).length,
        1,
      );
    for (const patch of [
      { driverKind: "claudeAgent" },
      { providerInstanceId: "other" },
      { options: { sandbox: "disabled" } },
    ]) {
      const input = delegate(model, "implementation", {
        reasoningEffort: model === "gpt-6.1-terra" ? "max" : "high",
      });
      Object.assign(input.target, patch);
      assert.equal(verdict(input).length, 1);
    }
  });
}
test("terra launch requires one exact implementation role line", () => {
  for (const suffix of [
    "\n- 역할: research",
    "\n- 역할: implementation",
    "\n- 역할: review",
  ]) {
    const input = launch("gpt-6.1-terra", "implementation", {
      reasoningEffort: "max",
    });
    input.message += suffix;
    assert.equal(verdict(input, true).length, 1);
  }
});

for (const model of ["gpt-6.1-sol", "gpt-6.1-terra"]) {
  test(`T3 reasoningEffort option ${model}`, () => {
    for (const reasoningEffort of ["high", "max", "low", "ultra"]) {
      const expected =
        reasoningEffort === (model === "gpt-6.1-terra" ? "max" : "high")
          ? 0
          : 1;
      assert.equal(
        verdict(delegate(model, "implementation", { reasoningEffort })).length,
        expected,
      );
      assert.equal(
        verdict(launch(model, "implementation", { reasoningEffort }), true)
          .length,
        expected,
      );
    }
    assert.equal(
      verdict(
        delegate(model, "implementation", {
          effort: "high",
          reasoningEffort: "max",
        }),
      ).length,
      1,
    );
  });
}

test("terra requires an explicit max option while sol retains omission", () => {
  for (const options of [
    undefined,
    {},
    [],
    null,
    { effort: undefined },
    { reasoningEffort: undefined },
  ]) {
    assert.equal(
      verdict(delegate("gpt-6.1-terra", "implementation", options)).length,
      1,
    );
    assert.equal(
      verdict(launch("gpt-6.1-terra", "implementation", options), true).length,
      1,
    );
  }
  for (const options of [undefined, {}, []]) {
    assert.deepEqual(
      verdict(delegate("gpt-6.1-sol", "implementation", options)),
      [],
    );
    assert.deepEqual(
      verdict(launch("gpt-6.1-sol", "implementation", options), true),
      [],
    );
  }
});

test("Opus review accepts effort high and rejects Codex reasoningEffort", () => {
  for (const [options, expected] of [
    [{ effort: "high" }, 0],
    [{ reasoningEffort: "high" }, 1],
    [{ reasoningEffort: "max" }, 1],
    [{ effort: "max" }, 1],
  ]) {
    const input = delegate("claude-opus-5-5", "review", options);
    input.target.driverKind = "claudeAgent";
    assert.equal(verdict(input).length, expected);
  }
});

test("model role and effort failures explain the rejected contract", () => {
  for (const role of ["research", "review", undefined]) {
    assert.match(
      verdict(delegate("gpt-6.1-terra", role, { reasoningEffort: "max" }))[0]
        .message,
      /모델.*역할/u,
    );
    assert.match(
      verdict(
        launch("gpt-6.1-terra", role, { reasoningEffort: "max" }),
        true,
      )[0].message,
      /모델.*역할/u,
    );
  }
  for (const options of [undefined, {}, [], { reasoningEffort: "high" }]) {
    assert.match(
      verdict(delegate("gpt-6.1-terra", "implementation", options))[0].message,
      /강도.*max/u,
    );
    assert.match(
      verdict(launch("gpt-6.1-terra", "implementation", options), true)[0]
        .message,
      /강도.*max/u,
    );
  }
});
