import { expect, test } from "vitest";
import { text } from "../lib";
import {
  settleInstrumentDetail,
  settleInstruments,
  settleWatchlistChange,
} from "./instruments-schema";
import { wire } from "./schema";
import { responses } from "./testing/responses";

test("settleInstruments keeps origin and leaves today empty when the backend has no rate", () => {
  const result = settleInstruments({
    status: "ok",
    data: wire(responses.getInstruments),
  });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  const [held, watched] = result.value.rows;
  expect(held?.origin).toBe("holding");
  expect(held?.todayRate === undefined).toBe(false);
  expect(held?.direction).toBe("up");
  expect(watched?.origin).toBe("watchlist");
  expect(watched?.todayRate === undefined).toBe(true);
});

test("settleInstrumentDetail carries the chart and the holding", () => {
  const result = settleInstrumentDetail({
    status: "ok",
    data: wire(responses.getInstrumentDetail),
  });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  expect(result.value.ticker).toBe(responses.getInstrumentDetail.symbol);
  expect(result.value.candles.map((candle) => candle.label)).toStrictEqual(
    responses.getInstrumentDetail.candles.map((candle) => candle.date),
  );
  expect(result.value.holding === undefined).toBe(false);
});

test("settleWatchlistChange turns a rejection code into the message text", () => {
  expect(
    settleWatchlistChange({ status: "ok", data: wire(responses.addWatchlist) }),
  ).toStrictEqual({ kind: "ok", value: { kind: "applied" } });
  expect(
    settleWatchlistChange({
      status: "ok",
      data: wire({ outcome: "rejected", reason: "unknown-instrument" }),
    }),
  ).toStrictEqual({
    kind: "ok",
    value: {
      kind: "rejected",
      reason: text("watchlist.unknown-instrument", "ko"),
    },
  });
  expect(
    settleWatchlistChange({
      status: "ok",
      data: wire({ outcome: "rejected", reason: null }),
    }).kind,
  ).toBe("err");
});
