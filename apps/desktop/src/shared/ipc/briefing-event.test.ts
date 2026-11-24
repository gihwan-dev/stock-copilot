import { expect, test } from "vitest";
import { parseUiEvent } from "./schema";

test("a finished briefing event becomes a briefing refresh", () => {
  expect(
    parseUiEvent({ BriefingFinished: { kind: "Morning", date: "2026-11-25" } }),
  ).toStrictEqual({
    kind: "ok",
    value: { category: "briefing", kind: "BriefingFinished" },
  });
  expect(
    parseUiEvent({ BriefingFinished: { kind: "Evening", date: "2026-11-25" } }),
  ).toStrictEqual({ kind: "err", error: { kind: "invalid-response" } });
  expect(
    parseUiEvent({ BriefingFinished: { kind: "Morning", date: "" } }),
  ).toStrictEqual({ kind: "err", error: { kind: "invalid-response" } });
});
