import { z } from "zod";
import {
  emptyFreeText,
  parseChartDecimal,
  parseEntityId,
  parseFreeText,
  text,
  type ChartDecimal,
  type EntityId,
  type FreeText,
  type Result,
  type UiText,
} from "../lib";
import type {
  ScorecardBinRow,
  ScorecardCombination,
  ScorecardScreenView,
} from "./estimate-views";
import { settleWithSchema, type IpcFailure } from "./schema";

type CommandOutcome =
  | { readonly status: "ok"; readonly data: unknown }
  | { readonly status: "error"; readonly error: unknown };

const asFreeText = (value: UiText | string): FreeText => {
  const parsed = parseFreeText(value);
  return parsed.kind === "ok" ? parsed.value : emptyFreeText;
};

const decimal = z.unknown().transform((value, context): ChartDecimal => {
  const parsed = parseChartDecimal(value);
  if (parsed.kind === "ok") return parsed.value;
  context.addIssue({ code: "custom", message: "decimal" });
  return z.NEVER;
});

const entity = (value: string): EntityId | undefined => {
  const parsed = parseEntityId(value);
  return parsed.kind === "ok" ? parsed.value : undefined;
};

const binKeys = {
  "below-50": "bin.below-50",
  "50-60": "bin.50-60",
  "60-70": "bin.60-70",
  "70-plus": "bin.70-plus",
} as const;

const horizonKeys = {
  hours: "horizon.hours",
  "few-days": "horizon.few-days",
  "weeks-to-months": "horizon.weeks-to-months",
  "half-year-or-more": "horizon.half-year-or-more",
} as const;

const binSchema = z.strictObject({
  bin: z.enum(["below-50", "50-60", "60-70", "70-plus"]),
  count: decimal,
  mean_prediction: z.union([decimal, z.null()]),
  hit_rate: z.union([decimal, z.null()]),
});

const combinationSchema = z.strictObject({
  model: z.string().min(1),
  horizon: z.enum([
    "hours",
    "few-days",
    "weeks-to-months",
    "half-year-or-more",
  ]),
  judged: decimal,
  verdict: z.enum(["pending", "ready"]),
  brier: z.union([decimal, z.null()]),
  bins: z.array(binSchema),
  comparisons: decimal,
});

type RawCombination = Readonly<z.infer<typeof combinationSchema>>;
type RawBin = Readonly<RawCombination["bins"][number]>;

const toBin = (
  row: Readonly<RawCombination>,
  point: RawBin,
): readonly ScorecardBinRow[] => {
  const binId = entity(`${row.model}:${row.horizon}:${point.bin}`);
  return binId === undefined ||
    point.mean_prediction === null ||
    point.hit_rate === null
    ? []
    : [
        {
          id: binId,
          label: asFreeText(text(binKeys[point.bin], "ko")),
          meanPrediction: point.mean_prediction,
          hitRate: point.hit_rate,
          count: point.count,
        },
      ];
};

const toCombination = (
  row: Readonly<RawCombination>,
): readonly ScorecardCombination[] => {
  const id = entity(`${row.model}:${row.horizon}`);
  return id === undefined
    ? []
    : [
        {
          id,
          model: asFreeText(row.model),
          horizon: asFreeText(text(horizonKeys[row.horizon], "ko")),
          judged: row.judged,
          verdict: row.verdict,
          brier: row.brier ?? undefined,
          bins: row.bins.flatMap((point) => toBin(row, point)),
          signalReturn: undefined,
          benchmarkReturn: undefined,
          intervalLow: undefined,
          intervalHigh: undefined,
          comparisons: row.comparisons,
        },
      ];
};

const viewSchema = z
  .strictObject({ combinations: z.array(combinationSchema) })
  .transform((raw): ScorecardScreenView => ({
    combinations: raw.combinations.flatMap(toCombination),
  }));

export const settleScorecard = (
  outcome: CommandOutcome,
): Result<ScorecardScreenView, IpcFailure> =>
  settleWithSchema(outcome, viewSchema);
