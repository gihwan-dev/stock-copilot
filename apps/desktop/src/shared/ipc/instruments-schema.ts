import { z } from "zod";
import {
  freeTextSchema,
  parseChartDecimal,
  parseEntityId,
  parseTicker,
  text,
  type ChartDecimal,
  type EntityId,
  type Result,
  type Ticker,
} from "../lib";
import type {
  InstrumentCandle,
  InstrumentDetailView,
  InstrumentListRow,
  InstrumentListView,
  WatchlistChange,
} from "./estimate-views";
import { settleWithSchema, type IpcFailure, type WireOutcome } from "./schema";
import {
  uiFreeText,
  wireMoney,
  wirePercent,
  wireQuantity,
  wireTimestamp,
} from "./wire-values";

const decimal = z.unknown().transform((value, context): ChartDecimal => {
  const parsed = parseChartDecimal(value);
  if (parsed.kind === "ok") return parsed.value;
  context.addIssue({ code: "custom", message: "decimal" });
  return z.NEVER;
});

const symbolSchema = z
  .unknown()
  .transform((value, context): { id: EntityId; ticker: Ticker } => {
    const id = parseEntityId(value);
    const ticker = parseTicker(value);
    if (id.kind === "ok" && ticker.kind === "ok")
      return { id: id.value, ticker: ticker.value };
    context.addIssue({ code: "custom", message: "symbol" });
    return z.NEVER;
  });

const todaySchema = z.union([
  z.strictObject({
    rate: wirePercent,
    direction: z.enum(["up", "down", "flat"]),
  }),
  z.null(),
]);

const rowSchema = z
  .strictObject({
    symbol: symbolSchema,
    name: freeTextSchema,
    origin: z.enum(["holding", "watchlist"]),
    today: todaySchema,
  })
  .transform((raw): InstrumentListRow => ({
    id: raw.symbol.id,
    name: raw.name,
    ticker: raw.symbol.ticker,
    origin: raw.origin,
    todayRate: raw.today?.rate,
    direction: raw.today?.direction ?? "flat",
  }));

const listSchema = z
  .strictObject({ rows: z.array(rowSchema) })
  .transform((raw): InstrumentListView => ({ rows: raw.rows }));

const candleSchema = z
  .strictObject({
    date: freeTextSchema,
    open: decimal,
    high: decimal,
    low: decimal,
    close: decimal,
  })
  .transform((raw, context): InstrumentCandle => {
    const id = parseEntityId(raw.date);
    if (id.kind === "err") {
      context.addIssue({ code: "custom", message: "date" });
      return z.NEVER;
    }
    return {
      id: id.value,
      label: raw.date,
      open: raw.open,
      high: raw.high,
      low: raw.low,
      close: raw.close,
    };
  });

const holdingSchema = z
  .strictObject({
    quantity: wireQuantity,
    value: wireMoney,
    profit: wireMoney,
    profit_rate: wirePercent,
  })
  .transform((raw) => ({
    quantity: raw.quantity,
    value: raw.value,
    profit: raw.profit,
    profitRate: raw.profit_rate,
  }));

const detailSchema = z
  .strictObject({
    symbol: symbolSchema,
    name: freeTextSchema,
    price: wireMoney,
    today: todaySchema,
    quoted_at: wireTimestamp,
    candles: z.array(candleSchema),
    holding: z.union([holdingSchema, z.null()]),
  })
  .transform((raw): InstrumentDetailView => ({
    id: raw.symbol.id,
    name: raw.name,
    ticker: raw.symbol.ticker,
    price: raw.price,
    todayRate: raw.today?.rate,
    direction: raw.today?.direction ?? "flat",
    quotedAt: raw.quoted_at,
    candles: raw.candles,
    holding: raw.holding ?? undefined,
  }));

const rejectionKeys = {
  "unknown-instrument": "watchlist.unknown-instrument",
  "inactive-instrument": "watchlist.inactive-instrument",
  "capacity-exceeded": "watchlist.capacity-exceeded",
  "not-in-watchlist": "watchlist.not-in-watchlist",
} as const;

const changeSchema = z
  .strictObject({
    outcome: z.enum(["applied", "rejected"]),
    reason: z.union([
      z.enum([
        "unknown-instrument",
        "inactive-instrument",
        "capacity-exceeded",
        "not-in-watchlist",
      ]),
      z.null(),
    ]),
  })
  .transform((raw, context): WatchlistChange => {
    if (raw.outcome === "applied") return { kind: "applied" };
    if (raw.reason === null) {
      context.addIssue({ code: "custom", message: "reason" });
      return z.NEVER;
    }
    return {
      kind: "rejected",
      reason: uiFreeText(text(rejectionKeys[raw.reason], "ko")),
    };
  });

export const settleInstruments = (
  outcome: WireOutcome,
): Result<InstrumentListView, IpcFailure> =>
  settleWithSchema(outcome, listSchema);

export const settleInstrumentDetail = (
  outcome: WireOutcome,
): Result<InstrumentDetailView, IpcFailure> =>
  settleWithSchema(outcome, detailSchema);

export const settleWatchlistChange = (
  outcome: WireOutcome,
): Result<WatchlistChange, IpcFailure> =>
  settleWithSchema(outcome, changeSchema);
