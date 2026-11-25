import { expect, test } from "vitest";
import { text } from "../lib";
import { wire } from "./schema";
import { responses } from "./testing/responses";
import { settleThesis } from "./thesis-schema";

const recorded = responses.getThesis;

test("settleThesis keeps claim review states and composes history summaries from codes", () => {
  const result = settleThesis({ status: "ok", data: wire(recorded) });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  expect(result.value.claims.map((claim) => claim.status)).toStrictEqual([
    "confirmed",
    "needs-review",
  ]);
  const [latest, decision, first] = result.value.history;
  expect(latest?.at === undefined).toBe(true);
  expect(first?.at === undefined).toBe(false);
  expect(decision?.summary).toBe(text("thesis.user-confirmed", "ko"));
  expect(first?.summary.startsWith(text("thesis.claim-added", "ko"))).toBe(
    true,
  );
});

test("settleThesis refuses an unknown change code", () => {
  const [row] = recorded.history;
  const result = settleThesis({
    status: "ok",
    data: wire({ ...recorded, history: [{ ...row, change: "deleted" }] }),
  });
  expect(result.kind).toBe("err");
});
