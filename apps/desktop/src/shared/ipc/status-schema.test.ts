import { expect, test } from "vitest";
import { text } from "../lib";
import { settleSelfCheck, settleStatus } from "./status-schema";
import { responses } from "./testing/responses";

test("settleSelfCheck shows one problem or clear", () => {
  expect(
    settleSelfCheck({ status: "ok", data: responses.getSelfCheck }),
  ).toStrictEqual({
    kind: "ok",
    value: { kind: "problem", problem: "claude-missing" },
  });
  expect(
    settleSelfCheck({ status: "ok", data: { problem: "none" } }),
  ).toStrictEqual({ kind: "ok", value: { kind: "clear" } });
  expect(
    settleSelfCheck({ status: "ok", data: { problem: "mcp-broken" } }).kind,
  ).toBe("err");
});

test("settleStatus names sources from messages and keeps the AI wait time", () => {
  const result = settleStatus({ status: "ok", data: responses.getStatus });
  expect(result.kind).toBe("ok");
  if (result.kind === "ok") {
    expect(result.value.aiRuntime).toStrictEqual({ kind: "ready" });
    expect(result.value.aiLimit.kind).toBe("waiting");
    expect(result.value.sources.map((row) => row.name)).toStrictEqual([
      text("source.kind-disclosure", "ko"),
      text("source.yonhap", "ko"),
    ]);
    expect(result.value.sources[0]?.lastSuccess).toBeUndefined();
  }
  expect(
    settleStatus({
      status: "ok",
      data: {
        ...responses.getStatus,
        sources: [{ source: "x", last_success: null }],
      },
    }).kind,
  ).toBe("err");
});
