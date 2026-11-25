import { expect, test } from "vitest";
import { readDecimalForNumberDisplay } from "../lib/money";
import { settleAllocation, settlePortfolio } from "./portfolio-schema";
import { wire } from "./schema";
import { responses } from "./testing/responses";

test("settlePortfolio turns the recorded core view into the overview with names and signed profit", () => {
  const result = settlePortfolio({
    status: "ok",
    data: wire(responses.getPortfolio),
  });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  const view = result.value;
  expect(view.recordedAt).toBe("2026-11-25T09:00:00+09:00");
  expect(view.totalCoverage).toBe("complete");
  expect(view.exchangeRate).toBeUndefined();
  expect(readDecimalForNumberDisplay(view.totalApproximate)).toBe("12200000");
  expect(view.holdings).toHaveLength(1);
  expect(view.holdings[0]?.name).toBe("삼성전자");
  expect(view.holdings[0]?.ticker).toBe("005930");
  expect(view.holdings[0]?.profit.direction).toBe("up");
  expect(readDecimalForNumberDisplay(view.holdings[0]?.profit.rate)).toBe(
    "10.77",
  );
  expect(view.today?.direction).toBe("down");
  expect(view.currencies.map((row) => row.currency)).toStrictEqual([
    "KRW",
    "USD",
  ]);
});

test("settlePortfolio rejects malformed money, extra fields and keeps command errors closed", () => {
  const recorded = responses.getPortfolio;
  expect(
    settlePortfolio({
      status: "ok",
      data: wire({ ...recorded, total: { amount: "1,000", currency: "KRW" } }),
    }),
  ).toStrictEqual({ kind: "err", error: { kind: "invalid-response" } });
  expect(
    settlePortfolio({
      status: "ok",
      data: wire({ ...recorded, account: "00000000731" }),
    }),
  ).toStrictEqual({ kind: "err", error: { kind: "invalid-response" } });
  expect(
    settlePortfolio({
      status: "ok",
      data: wire({ ...recorded, today: null, exchange_rate_at: null }),
    }).kind,
  ).toBe("ok");
  expect(
    settlePortfolio({ status: "error", error: wire({ kind: "no-data" }) }),
  ).toStrictEqual({
    kind: "err",
    error: { kind: "command", error: { kind: "no-data" } },
  });
});

test("settleAllocation keeps the cash row kind so the screen names it, and reads weights", () => {
  const result = settleAllocation({
    status: "ok",
    data: wire(responses.getAllocation),
  });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  expect(result.value.concentration).toBe("concentrated");
  expect(result.value.sectors).toStrictEqual([]);
  expect(result.value.instruments.map((row) => row.kind)).toStrictEqual([
    "holding",
    "cash-krw",
  ]);
  expect(result.value.instruments[0]?.level).toBe("over-cap");
  expect(readDecimalForNumberDisplay(result.value.instruments[0]?.weight)).toBe(
    "59.02",
  );
});
