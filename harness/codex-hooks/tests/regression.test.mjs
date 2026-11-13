import { strict as assert } from "node:assert";
import { test } from "node:test";
import { experiments, judge } from "../../codex-regression/run.mjs";
test("regression requires every actual attempt and fails on a successful retry", () => {
  const spec = experiments.map((item) => ({ ...item, digest: item.id }));
  const events = spec.map((item) => ({
    inputDigest: item.digest,
    decision: item.decision ?? "deny",
  }));
  const states = Object.fromEntries(
    spec.map((item) => [item.id, item.decision ?? "deny"]),
  );
  assert.equal(
    judge(spec, events, states).filter((row) => row.status === "통과").length,
    10,
  );
  for (const [index] of events.entries())
    assert.ok(
      judge(
        spec,
        events.filter((_, other) => index !== other),
        states,
      ).some((row) => row.status !== "통과"),
    );
  assert.equal(
    judge(spec, [...events, { ...events[0], decision: "allow" }], states)[0]
      .status,
    "실패",
  );
  assert.equal(
    judge(spec, events, { ...states, C1: "allow" })[0].status,
    "실패",
  );
});
