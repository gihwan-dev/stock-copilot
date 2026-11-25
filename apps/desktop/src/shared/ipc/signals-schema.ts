import { z } from "zod";
import {
  freeTextSchema,
  parseChartDecimal,
  parseEntityId,
  parseTicker,
  type ChartDecimal,
  type EntityId,
  type Result,
  type Ticker,
} from "../lib";
import type {
  SignalCardView,
  SignalSource,
  SignalsView,
} from "./estimate-views";
import { settleWithSchema, type IpcFailure, type WireOutcome } from "./schema";
import { wireMoney, wirePercent } from "./wire-values";

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

const proposalSchema = z.unknown().transform((value, context): EntityId => {
  const id = parseEntityId(value);
  if (id.kind === "ok") return id.value;
  context.addIssue({ code: "custom", message: "proposal" });
  return z.NEVER;
});

const sourceSchema = z.strictObject({
  kind: z.enum(["tool", "web", "news", "disclosure"]),
  label: freeTextSchema,
});

const cardSchema = z
  .strictObject({
    symbol: symbolSchema,
    name: freeTextSchema,
    direction: z.enum(["buy", "sell"]),
    market: z.enum(["open", "closed"]),
    order_switch: z.enum(["on", "off"]),
    price: z.union([wireMoney, z.null()]),
    ai_probability: wirePercent,
    tracked_probability: wirePercent,
    threshold: wirePercent,
    bin_hit_rate: z.union([wirePercent, z.null()]),
    bin_count: decimal,
    rationale: freeTextSchema,
    counterargument: freeTextSchema,
    sources: z.array(sourceSchema),
    proposal: z.union([proposalSchema, z.null()]),
  })
  .transform((raw, context): SignalCardView => {
    const cited = raw.sources.flatMap(
      (source, index): readonly SignalSource[] => {
        const id = parseEntityId(`${raw.symbol.id}:${String(index)}`);
        return id.kind === "ok"
          ? [{ id: id.value, kind: source.kind, label: source.label }]
          : [];
      },
    );
    if (cited.length !== raw.sources.length) {
      context.addIssue({ code: "custom", message: "source" });
      return z.NEVER;
    }
    return {
      id: raw.symbol.id,
      name: raw.name,
      ticker: raw.symbol.ticker,
      direction: raw.direction,
      market: raw.market,
      orderSwitch: raw.order_switch,
      proposal: raw.proposal ?? undefined,
      price: raw.price ?? undefined,
      sizing: { kind: "pending" },
      aiProbability: raw.ai_probability,
      trackedProbability: raw.tracked_probability,
      threshold: raw.threshold,
      binHitRate: raw.bin_hit_rate ?? undefined,
      binCount: raw.bin_count,
      rationale: raw.rationale,
      counterargument: raw.counterargument,
      sources: cited,
    };
  });

const viewSchema = z
  .strictObject({ cards: z.array(cardSchema) })
  .transform((raw): SignalsView => ({ cards: raw.cards }));

const approvalSchema = z
  .strictObject({
    status: z.enum([
      "submitted",
      "rejected",
      "broker-rejected",
      "outcome-unknown",
      "unavailable",
    ]),
    reason: z.string(),
  })
  .transform((raw) => raw.status);

const skipSchema = z
  .strictObject({ outcome: z.literal("recorded") })
  .transform((): "recorded" => "recorded");

export const settleSignals = (
  outcome: WireOutcome,
): Result<SignalsView, IpcFailure> => settleWithSchema(outcome, viewSchema);

export const settleSkip = (
  outcome: WireOutcome,
): Result<"recorded", IpcFailure> => settleWithSchema(outcome, skipSchema);

export const settleApproval = (
  outcome: WireOutcome,
): Result<"requested", IpcFailure> => {
  const settled = settleWithSchema(outcome, approvalSchema);
  if (settled.kind === "err") return settled;
  return settled.value === "submitted" || settled.value === "outcome-unknown"
    ? { kind: "ok", value: "requested" }
    : {
        kind: "err",
        error: { kind: "command", error: { kind: "user-fixable" } },
      };
};
