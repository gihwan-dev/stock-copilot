import { z } from "zod";
import {
  parsePercentDisplay,
  parseTimestamp,
  type PercentDisplay,
  type Result,
  type Timestamp,
} from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import {
  settleWithSchema,
  wireOutcome,
  type IpcFailure,
  type WireOutcome,
} from "./schema";

export type PerformanceSpan = "1m" | "3m" | "6m" | "1y";

export type SignedRate = {
  readonly rate: PercentDisplay;
  readonly direction: "up" | "down" | "flat";
};

export type PerformanceLine = {
  readonly span: PerformanceSpan;
  readonly status: z.infer<typeof statusSchema>;
  readonly periodReturn: SignedRate | undefined;
  readonly maxDrawdown: SignedRate | undefined;
  readonly from: Timestamp | undefined;
  readonly to: Timestamp | undefined;
};

export type PerformanceView = {
  readonly lines: readonly PerformanceLine[];
};

const percentSchema = z.string().transform((raw, context): PercentDisplay => {
  const parsed = parsePercentDisplay(raw);
  if (parsed.kind === "ok") return parsed.value;
  context.addIssue({ code: "custom", message: "percent" });
  return z.NEVER;
});

const timestampSchema = z.unknown().transform((value, context) => {
  const parsed = parseTimestamp(value);
  if (parsed.kind === "ok") return parsed.value;
  context.addIssue({ code: "custom", message: "timestamp" });
  return z.NEVER;
});

const optionalTimestamp = z
  .union([timestampSchema, z.null()])
  .transform((value): Timestamp | undefined => value ?? undefined);

const rateSchema = z
  .union([
    z.strictObject({
      rate: percentSchema,
      direction: z.enum(["up", "down", "flat"]),
    }),
    z.null(),
  ])
  .transform((value): SignedRate | undefined => value ?? undefined);

const statusSchema = z.enum([
  "waiting",
  "calculating",
  "ready",
  "missing-price",
  "missing-fx",
  "missing-cash",
  "insufficient-data",
  "load-failed",
  "non-positive-capital",
  "unavailable",
]);

const performanceSchema = z
  .strictObject({
    lines: z.array(
      z.strictObject({
        span: z.enum(["1m", "3m", "6m", "1y"]),
        status: statusSchema.optional(),
        period_return: rateSchema,
        max_drawdown: rateSchema,
        from: optionalTimestamp,
        to: optionalTimestamp,
      }),
    ),
  })
  .transform((raw): PerformanceView => ({
    lines: raw.lines.map((line): PerformanceLine => ({
      span: line.span,
      status:
        line.status ??
        (line.period_return === undefined ? "unavailable" : "ready"),
      periodReturn: line.period_return,
      maxDrawdown: line.max_drawdown,
      from: line.from,
      to: line.to,
    })),
  }));

export const settlePerformance = (
  outcome: WireOutcome,
): Result<PerformanceView, IpcFailure> =>
  settleWithSchema(outcome, performanceSchema);

export const getPerformance = (): Promise<
  Result<PerformanceView, IpcFailure>
> =>
  settleCall(commands.getPerformance({ scope: "current" }), (outcome) =>
    settlePerformance(wireOutcome(outcome)),
  );
