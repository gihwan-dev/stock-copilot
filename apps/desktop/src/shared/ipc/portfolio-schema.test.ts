import { expect, test } from "vitest";
import { settleAllocation, settlePortfolio } from "./portfolio-schema";
import { wire } from "./schema";
import { responses } from "./testing/responses";

const recorded = responses.getPortfolio;
const recordedHolding = recorded.holdings[0];

test("settlePortfolio keeps the recorded time, coverage and currency rows", () => {
  const result = settlePortfolio({ status: "ok", data: wire(recorded) });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  const view = result.value;
  expect(view.recordedAt).toBe(recorded.recorded_at);
  expect(view.totalCoverage).toBe("complete");
  expect(view.exchangeRate).toBeUndefined();
  expect(view.today?.direction).toBe("down");
  expect(view.currencies.map((row) => row.currency)).toStrictEqual([
    "KRW",
    "USD",
  ]);
});

test("settlePortfolio carries the master name, symbol and signed profit of each holding", () => {
  const result = settlePortfolio({ status: "ok", data: wire(recorded) });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  const holding = result.value.holdings[0];
  expect(holding?.name).toBe(recordedHolding.name);
  expect(holding?.ticker).toBe(recordedHolding.symbol);
  expect(holding?.profit.direction).toBe("up");
  expect(holding?.trading).toBe("trading");
});

test("settlePortfolio keeps a suspended holding as a row marked suspended", () => {
  const result = settlePortfolio({
    status: "ok",
    data: wire({
      ...recorded,
      holdings: [{ ...recordedHolding, trading: "suspended" }],
    }),
  });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  expect(result.value.holdings[0]?.trading).toBe("suspended");
});

test("settlePortfolio rejects malformed money and extra fields", () => {
  const invalid = { kind: "err", error: { kind: "invalid-response" } };
  expect(
    settlePortfolio({
      status: "ok",
      data: wire({ ...recorded, total: { amount: "1,000", currency: "KRW" } }),
    }),
  ).toStrictEqual(invalid);
  expect(
    settlePortfolio({
      status: "ok",
      data: wire({ ...recorded, account: "00000000731" }),
    }),
  ).toStrictEqual(invalid);
});

test("settlePortfolio accepts a missing daily profit and keeps command errors closed", () => {
  const partial = settlePortfolio({
    status: "ok",
    data: wire({ ...recorded, today: null, exchange_rate_at: null }),
  });
  expect(partial.kind).toBe("ok");
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
});
