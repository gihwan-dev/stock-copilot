import { z } from "zod";
import {
  chatIdSchema,
  parseEntityId,
  freeTextSchema,
  parseMoney,
  parsePercentDisplay,
  parseQuantity,
  parseTimestamp,
  type Money,
  type PercentDisplay,
  type Quantity,
  type Result,
  type Timestamp,
} from "../lib";
import type {
  AllocationRow,
  AllocationView,
  CurrencyValue,
  HoldingRow,
  PortfolioView,
} from "./screen-sources";
import { settleWithSchema, type IpcFailure, type WireOutcome } from "./schema";

const timestampSchema = z.unknown().transform((value, context): Timestamp => {
  const parsed = parseTimestamp(value);
  if (parsed.kind === "ok") return parsed.value;
  context.addIssue({ code: "custom", message: "timestamp" });
  return z.NEVER;
});

const moneySchema = z
  .strictObject({ amount: z.string(), currency: z.enum(["KRW", "USD"]) })
  .transform((raw, context): Money => {
    const parsed = parseMoney(raw.amount, raw.currency);
    if (parsed.kind === "ok") return parsed.value;
    context.addIssue({ code: "custom", message: "money" });
    return z.NEVER;
  });

const percentSchema = z.string().transform((raw, context): PercentDisplay => {
  const parsed = parsePercentDisplay(raw);
  if (parsed.kind === "ok") return parsed.value;
  context.addIssue({ code: "custom", message: "percent" });
  return z.NEVER;
});

const quantitySchema = z.string().transform((raw, context): Quantity => {
  const parsed = parseQuantity(raw);
  if (parsed.kind === "ok") return parsed.value;
  context.addIssue({ code: "custom", message: "quantity" });
  return z.NEVER;
});

const signedSchema = z.strictObject({
  amount: moneySchema,
  rate: percentSchema,
  direction: z.enum(["up", "down", "flat"]),
});

const holdingSchema = z
  .strictObject({
    symbol: z.string().min(1).max(32),
    name: freeTextSchema,
    trading: z.enum(["trading", "suspended"]),
    quantity: quantitySchema,
    value: moneySchema,
    profit: signedSchema,
  })
  .transform((raw, context): HoldingRow => {
    const id = chatIdSchema.safeParse(raw.symbol);
    const ticker = freeTextSchema.safeParse(raw.symbol);
    if (!id.success || !ticker.success) {
      context.addIssue({ code: "custom", message: "symbol" });
      return z.NEVER;
    }
    return {
      id: id.data,
      name: raw.name,
      ticker: ticker.data,
      trading: raw.trading,
      quantity: raw.quantity,
      value: raw.value,
      profit: raw.profit,
    };
  });

const currencySchema = z
  .strictObject({
    currency: z.enum(["KRW", "USD"]),
    holdings: moneySchema,
    buying_power: z.union([moneySchema, z.null()]),
  })
  .transform((raw): CurrencyValue => ({
    currency: raw.currency,
    holdings: raw.holdings,
    buyingPower: raw.buying_power ?? undefined,
  }));

const portfolioSchema = z
  .strictObject({
    recorded_at: timestampSchema,
    total: moneySchema,
    total_coverage: z.enum(["complete", "partial"]),
    exchange_rate_at: z.union([timestampSchema, z.null()]),
    currencies: z.array(currencySchema),
    holdings: z.array(holdingSchema),
    today: z.union([signedSchema, z.null()]),
  })
  .transform((raw): PortfolioView => ({
    recordedAt: raw.recorded_at,
    connection: "live",
    totalApproximate: raw.total,
    totalCoverage: raw.total_coverage,
    exchangeRate:
      raw.exchange_rate_at === null
        ? undefined
        : { kind: "mid-rate", at: raw.exchange_rate_at },
    currencies: raw.currencies,
    holdings: raw.holdings,
    today: raw.today ?? undefined,
  }));

const weightLevelSchema = z.enum(["normal", "warning", "over-cap"]);

const allocationRowSchema = z.strictObject({
  id: chatIdSchema,
  kind: z.enum(["holding", "cash-krw", "cash-usd"]),
  name: freeTextSchema,
  value: moneySchema,
  weight: percentSchema,
  weight_range: z.enum(["below-unit", "at-least-unit"]),
  level: weightLevelSchema,
});

const sectorRowSchema = allocationRowSchema.extend({
  id: z
    .string()
    .min(1)
    .max(128)
    .transform((value, context) => {
      const id = parseEntityId(value);
      if (id.kind === "ok") return id.value;
      context.addIssue({ code: "custom", message: "sector" });
      return z.NEVER;
    }),
  kind: z.enum(["sector", "etf-fund", "cash", "unclassified"]),
  level: weightLevelSchema.nullable(),
});

const allocationRow = (
  raw: Readonly<
    z.output<typeof allocationRowSchema> | z.output<typeof sectorRowSchema>
  >,
): AllocationRow => ({
  id: raw.id,
  kind: raw.kind,
  label: raw.name,
  value: raw.value,
  weight: raw.weight,
  weightRange: raw.weight_range,
  level: raw.level,
});

const allocationSchema = z
  .strictObject({
    instruments: z.array(allocationRowSchema.transform(allocationRow)),
    sectors: z.array(sectorRowSchema.transform(allocationRow)),
    sector_refresh: z.enum(["pending", "settled"]),
    concentration: z.enum(["balanced", "concentrated"]),
  })
  .transform((raw): AllocationView => ({
    instruments: raw.instruments,
    sectors: raw.sectors,
    sectorRefresh: raw.sector_refresh,
    concentration: raw.concentration,
  }));

export const settlePortfolio = (
  outcome: WireOutcome,
): Result<PortfolioView, IpcFailure> =>
  settleWithSchema(outcome, portfolioSchema);

export const settleAllocation = (
  outcome: WireOutcome,
): Result<AllocationView, IpcFailure> =>
  settleWithSchema(outcome, allocationSchema);
