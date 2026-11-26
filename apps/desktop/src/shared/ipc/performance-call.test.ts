import { expect, test } from "vitest";
import { settlePerformance } from "./performance-call";
import { wire } from "./schema";
import { responses } from "./testing/responses";

test("settlePerformance lists every period and keeps unknown numbers absent", () => {
  const result = settlePerformance({
    status: "ok",
    data: wire(responses.getPerformance),
  });
  expect(result.kind).toBe("ok");
  if (result.kind === "ok") {
    expect(result.value.lines.map((line) => line.span)).toStrictEqual([
      "1m",
      "3m",
      "6m",
      "1y",
    ]);
    expect(result.value.lines[0]?.periodReturn).toBeUndefined();
  }
  const filled = settlePerformance({
    status: "ok",
    data: wire({
      lines: [
        {
          span: "1y",
          period_return: { rate: "-5.00", direction: "down" },
          max_drawdown: { rate: "-9.52", direction: "down" },
          from: "2025-11-26T16:00:00+09:00",
          to: "2026-11-26T16:00:00+09:00",
        },
      ],
    }),
  });
  expect(filled.kind).toBe("ok");
  if (filled.kind === "ok") {
    expect(filled.value.lines[0]?.periodReturn?.direction).toBe("down");
  }
  expect(
    settlePerformance({ status: "ok", data: wire({ lines: [{ span: "2y" }] }) })
      .kind,
  ).toBe("err");
});
