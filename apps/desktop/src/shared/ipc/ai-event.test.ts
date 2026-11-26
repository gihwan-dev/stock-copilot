import { expect, test } from "vitest";
import { parseUiEvent } from "./schema";

test("AI command and approval events refresh every screen", () => {
  expect(parseUiEvent({ AiCommandApplied: "set_principles" })).toStrictEqual({
    kind: "ok",
    value: { category: "all", kind: "AiCommandApplied" },
  });
  expect(
    parseUiEvent({ AiApprovalRequested: "p-20261126-0001" }),
  ).toStrictEqual({
    kind: "ok",
    value: { category: "all", kind: "AiApprovalRequested" },
  });
  expect(parseUiEvent({ AiCommandApplied: "" })).toStrictEqual({
    kind: "err",
    error: { kind: "invalid-response" },
  });
});
