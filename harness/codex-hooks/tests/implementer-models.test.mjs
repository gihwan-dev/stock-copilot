import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { orchestrationFindings } from "../../../.claude/hooks/lib/orchestration.mjs";
import { knownGaps } from "../../known-gaps/list.mjs";
import { repoRoot } from "../../claude-hooks/tests/fixture.mjs";
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
const delegate = (model, role, options, targetExtra = {}) => ({
  isolation: "worktree",
  target: {
    driverKind: "antigravity",
    providerInstanceId: "antigravity",
    model,
    options,
    ...targetExtra,
  },
  role,
  task: brief + (role === "review" ? `\n- 대상 SHA: ${"b".repeat(40)}` : ""),
});
const launch = (model, role, options, selectionExtra = {}) => ({
  modelSelection: {
    provider: "antigravity",
    model,
    options,
    ...selectionExtra,
  },
  workspaceStrategy: {
    type: "worktree",
    branch: "topic",
    baseRef: base,
    startFromOrigin: false,
  },
  message: brief + (role === undefined ? "" : `\n- 역할: ${role}`),
});
for (const model of [
  "gemini-4-neon",
  "gemini-4-carbon",
  "gemini-4-argon",
  "gemini-4-hydrogen",
  "gemini-3.8-flash",
  "gemini-4-neon-extra",
  "gpt-6.1-sol",
  "gpt-6.1-terra",
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
    const level = role === "implementation" ? "xhigh" : "high";
    test(`delegate exact model ${model} role ${role}`, () => {
      const accepted =
        model === "gemini-4-neon" &&
        ["implementation", "research", "review"].includes(role);
      assert.equal(
        verdict(delegate(model, role, { thought_level: level })).length,
        accepted ? 0 : 1,
      );
    });
    test(`launch exact model ${model} role ${role}`, () => {
      const accepted = model === "gemini-4-neon" && role === "implementation";
      assert.equal(
        verdict(launch(model, role, { thought_level: level }), true).length,
        accepted ? 0 : 1,
      );
    });
  }
}
for (const role of ["implementation", "research", "review"]) {
  const expectedLevel = role === "implementation" ? "xhigh" : "high";
  for (const level of ["low", "medium", "high", "xhigh", "max", "ultra"]) {
    test(`Antigravity thought_level ${role} ${level}`, () => {
      const expected = level === expectedLevel ? 0 : 1;
      assert.equal(
        verdict(delegate("gemini-4-neon", role, { thought_level: level }))
          .length,
        expected,
      );
      if (role === "implementation") {
        assert.equal(
          verdict(launch("gemini-4-neon", role, { thought_level: level }), true)
            .length,
          expected,
        );
      }
    });
  }
}
test("Antigravity boundaries reject non-worktree, elevated mode, wrong provider, wrong instance, and foreign option keys", () => {
  for (const patch of [{ isolation: "shared" }, { runtimeMode: "full-access" }])
    assert.equal(
      verdict({
        ...delegate("gemini-4-neon", "implementation", {
          thought_level: "xhigh",
        }),
        ...patch,
      }).length,
      1,
    );
  for (const patch of [
    { driverKind: "codex" },
    { driverKind: "claudeAgent" },
    { driverKind: "cursor" },
    { providerInstanceId: "other" },
    { providerInstanceId: "codex" },
    { providerInstanceId: "" },
    { providerInstanceId: null },
    { options: { sandbox: "disabled" } },
    { options: { effort: "xhigh" } },
    { options: { reasoningEffort: "max" } },
  ]) {
    const input = delegate("gemini-4-neon", "implementation", {
      thought_level: "xhigh",
    });
    Object.assign(input.target, patch);
    assert.equal(verdict(input).length, 1);
  }
  assert.deepEqual(
    verdict(
      delegate(
        "gemini-4-neon",
        "implementation",
        { thought_level: "xhigh" },
        { providerInstanceId: undefined },
      ),
    ),
    [],
  );
  assert.deepEqual(
    verdict(
      launch(
        "gemini-4-neon",
        "implementation",
        { thought_level: "xhigh" },
        { instanceId: "antigravity" },
      ),
      true,
    ),
    [],
  );
  for (const instanceId of ["other", "codex", "claudeAgent_claude2", "", null])
    assert.equal(
      verdict(
        launch(
          "gemini-4-neon",
          "implementation",
          { thought_level: "xhigh" },
          { instanceId },
        ),
        true,
      ).length,
      1,
    );
});
test("Codex provider and models are rejected for both delegate and launch", () => {
  for (const model of ["gpt-6.1-sol", "gpt-6.1-terra", "gpt-6.2-sol"]) {
    for (const options of [
      undefined,
      { effort: "high" },
      { reasoningEffort: "high" },
      { reasoningEffort: "max" },
      { thought_level: "xhigh" },
    ]) {
      const delegated = verdict({
        isolation: "worktree",
        target: { driverKind: "codex", model, options },
        role: "implementation",
        task: brief,
      });
      assert.equal(delegated.length, 1);
      assert.match(delegated[0].message, /Codex.*허용하지 않는다/u);
      const launched = verdict(
        {
          modelSelection: { provider: "codex", model, options },
          workspaceStrategy: {
            type: "worktree",
            branch: "topic",
            baseRef: base,
            startFromOrigin: false,
          },
          message: `${brief}\n- 역할: implementation`,
        },
        true,
      );
      assert.equal(launched.length, 1);
      assert.match(launched[0].message, /Codex.*허용하지 않는다/u);
    }
  }
});
test("Antigravity launch requires one exact implementation role line", () => {
  for (const suffix of [
    "\n- 역할: research",
    "\n- 역할: implementation",
    "\n- 역할: review",
  ]) {
    const input = launch("gemini-4-neon", "implementation", {
      thought_level: "xhigh",
    });
    input.message += suffix;
    assert.equal(verdict(input, true).length, 1);
  }
  const missingRole = launch("gemini-4-neon", undefined, {
    thought_level: "xhigh",
  });
  assert.equal(verdict(missingRole, true).length, 1);
  const nonStringMessage = launch("gemini-4-neon", "implementation", {
    thought_level: "xhigh",
  });
  nonStringMessage.message = {
    toString: () => `${brief}\n- 역할: implementation`,
  };
  assert.equal(verdict(nonStringMessage, true).length, 1);
});
test("Antigravity requires an explicit thought_level option and rejects omission or empty values", () => {
  const arrayWithProp = Object.assign([], { thought_level: "xhigh" });
  const inheritedOption = Object.assign(
    Object.create({ thought_level: "xhigh" }),
    { decoy: 1 },
  );
  for (const options of [
    undefined,
    {},
    [],
    null,
    arrayWithProp,
    inheritedOption,
    { thought_level: undefined },
    { effort: "high" },
    { reasoningEffort: "high" },
  ]) {
    for (const role of ["implementation", "research", "review"])
      assert.equal(verdict(delegate("gemini-4-neon", role, options)).length, 1);
    assert.equal(
      verdict(launch("gemini-4-neon", "implementation", options), true).length,
      1,
    );
  }
  assert.equal(
    orchestrationFindings(
      { tool_name: "mcp__t3-code__t3_project_list", tool_input: [] },
      rules,
      facts,
    ).length,
    1,
  );
});
test("Claude2 Opus review requires claudeAgent_claude2 and accepts only omission or effort high", () => {
  for (const [options, expected] of [
    [undefined, 0],
    [{ effort: "high" }, 0],
    [{}, 1],
    [[], 1],
    [null, 1],
    [{ effort: undefined }, 1],
    [{ effort: "max" }, 1],
    [{ effort: "medium" }, 1],
    [{ reasoningEffort: "high" }, 1],
    [{ thought_level: "high" }, 1],
    [{ thought_level: "xhigh" }, 1],
    [{ effort: "high", thought_level: "high" }, 1],
    [{ ultracode: true }, 1],
    [{ fastMode: true }, 1],
  ]) {
    const input = delegate("claude-opus-5-5", "review", options, {
      driverKind: "claudeAgent",
      providerInstanceId: "claudeAgent_claude2",
    });
    assert.equal(verdict(input).length, expected);
  }
  for (const providerInstanceId of [
    undefined,
    "claudeAgent",
    "other",
    "",
    null,
  ]) {
    const input = delegate(
      "claude-opus-5-5",
      "review",
      { effort: "high" },
      { driverKind: "claudeAgent", providerInstanceId },
    );
    const findings = verdict(input);
    assert.equal(findings.length, 1);
    assert.match(findings[0].message, /claudeAgent_claude2/u);
  }
  for (const role of ["implementation", "research", undefined]) {
    const input = delegate(
      "claude-opus-5-5",
      role,
      { effort: "high" },
      {
        driverKind: "claudeAgent",
        providerInstanceId: "claudeAgent_claude2",
      },
    );
    assert.equal(verdict(input).length, 1);
  }
  assert.equal(
    verdict(
      launch(
        "claude-opus-5-5",
        "implementation",
        { effort: "high" },
        { provider: "claudeAgent", instanceId: "claudeAgent_claude2" },
      ),
      true,
    ).length,
    1,
  );
});
test("model role and option failures explain the rejected contract", () => {
  for (const role of ["general", "critic", undefined]) {
    assert.match(
      verdict(delegate("gemini-4-neon", role, { thought_level: "high" }))[0]
        .message,
      /gemini-4-neon.*역할/u,
    );
  }
  for (const role of ["research", "review", undefined]) {
    assert.match(
      verdict(
        launch("gemini-4-neon", role, { thought_level: "xhigh" }),
        true,
      )[0].message,
      /gemini-4-neon.*역할: implementation/u,
    );
  }
  for (const options of [undefined, {}, [], { thought_level: "high" }]) {
    assert.match(
      verdict(delegate("gemini-4-neon", "implementation", options))[0].message,
      /thought_level=xhigh/u,
    );
    assert.match(
      verdict(launch("gemini-4-neon", "implementation", options), true)[0]
        .message,
      /thought_level=xhigh/u,
    );
  }
  assert.match(
    verdict(delegate("gemini-4-neon", "review", { thought_level: "xhigh" }))[0]
      .message,
    /thought_level=high/u,
  );
});
test("mixed option keys are denied even when both values match", () => {
  for (const [role, level] of [
    ["implementation", "xhigh"],
    ["research", "high"],
    ["review", "high"],
  ]) {
    const options = { thought_level: level, effort: level };
    assert.equal(verdict(delegate("gemini-4-neon", role, options)).length, 1);
  }
  assert.equal(
    verdict(
      launch("gemini-4-neon", "implementation", {
        thought_level: "xhigh",
        reasoningEffort: "max",
      }),
      true,
    ).length,
    1,
  );
});
test("Antigravity launch role marker must start its own line", () => {
  for (const prefix of ["prefix", " ", "* "]) {
    const input = launch("gemini-4-neon", "implementation", {
      thought_level: "xhigh",
    });
    input.message = input.message.replace("\n- 역할:", `\n${prefix}- 역할:`);
    assert.equal(verdict(input, true).length, 1);
  }
});
test("diagnostics list exact Antigravity model and prohibit mixed option keys", () => {
  const model = verdict(
    delegate("gemini-4-carbon", "implementation", { thought_level: "xhigh" }),
  )[0].message;
  assert.match(model, /gemini-4-neon/u);
  const keys = verdict(
    delegate("gemini-4-neon", "implementation", {
      thought_level: "xhigh",
      effort: "high",
    }),
  )[0].message;
  assert.match(keys, /thought_level/u);
  assert.match(keys, /effort/u);
  assert.match(keys, /혼합/u);
});
test("known-gaps records Antigravity session enforcement gaps", () => {
  const gaps = knownGaps(repoRoot).filter((gap) => gap.source === "adr");
  assert.ok(
    gaps.some(
      (gap) =>
        gap.text.includes("Antigravity") &&
        gap.text.includes("보호 경로 쓰기 차단"),
    ),
  );
  assert.ok(
    gaps.some(
      (gap) => gap.text.includes("Antigravity") && gap.text.includes("홈 파일"),
    ),
  );
  assert.ok(
    gaps.some(
      (gap) =>
        gap.text.includes("Antigravity") && gap.text.includes("리뷰·비판 역할"),
    ),
  );
  assert.ok(
    gaps.some(
      (gap) =>
        gap.text.includes("Antigravity") &&
        gap.text.includes("종료 시 전체 검사"),
    ),
  );
});
