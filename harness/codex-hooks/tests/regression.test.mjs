import { strict as assert } from "node:assert";
import { test } from "node:test";
import { experiments, judge } from "../../codex-regression/run.mjs";

test("regression requires every actual attempt and fails on a successful retry", () => {
  const events = experiments.map((item) => ({
    event: "PreToolUse",
    input: {
      command:
        item.command ??
        `*** Begin Patch\n*** Add File: ${item.path}\n+probe\n*** End Patch`,
    },
    output:
      item.decision === "allow"
        ? undefined
        : { hookSpecificOutput: { permissionDecision: "deny" } },
  }));
  assert.equal(
    judge(experiments, events).filter((row) => row.status === "통과").length,
    10,
  );
  for (const [index] of events.entries())
    assert.equal(
      judge(
        experiments,
        events.filter((_, other) => other !== index),
      ).some((row) => row.status !== "통과"),
      true,
    );
  assert.equal(
    judge(experiments, [...events, { ...events[0], output: undefined }])[0]
      .status,
    "실패",
  );
});
