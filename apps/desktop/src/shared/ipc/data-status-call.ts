import { z } from "zod";
import { parseTimestamp, type Result, type Timestamp } from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import {
  settleWithSchema,
  wireOutcome,
  type IpcFailure,
  type WireOutcome,
} from "./schema";

export type DataJob =
  | "holdings"
  | "transactions"
  | "daily-valuation"
  | "estimates"
  | "kr-scoring"
  | "us-scoring"
  | "morning-briefing"
  | "closing-review"
  | "pre-us-market-check"
  | "backup";

const dataJobs = [
  "holdings",
  "transactions",
  "daily-valuation",
  "estimates",
  "kr-scoring",
  "us-scoring",
  "morning-briefing",
  "closing-review",
  "pre-us-market-check",
  "backup",
] as const satisfies readonly DataJob[];

export type DataRunOutcome = "succeeded" | "skipped" | "waiting" | "failed";

export type DataJobRun = {
  readonly at: Timestamp;
  readonly outcome: DataRunOutcome;
};

export type DataJobStatus = {
  readonly job: DataJob;
  readonly lastSuccess: Timestamp | undefined;
  readonly lastRun: DataJobRun | undefined;
  readonly nextRun: Timestamp | undefined;
};

export type DataStatusView = {
  readonly jobs: readonly DataJobStatus[];
};

const timestampSchema = z.unknown().transform((value, context) => {
  const parsed = parseTimestamp(value);
  if (parsed.kind === "ok") return parsed.value;
  context.addIssue({ code: "custom", message: "timestamp" });
  return z.NEVER;
});

const optionalTimestamp = z
  .union([timestampSchema, z.null()])
  .transform((value): Timestamp | undefined => value ?? undefined);

const runSchema = z
  .union([
    z.strictObject({
      at: timestampSchema,
      outcome: z.enum(["succeeded", "skipped", "waiting", "failed"]),
    }),
    z.null(),
  ])
  .transform((value): DataJobRun | undefined => value ?? undefined);

const dataStatusSchema = z
  .strictObject({
    jobs: z.array(
      z.strictObject({
        job: z.enum(dataJobs),
        last_success: optionalTimestamp,
        last_run: runSchema,
        next_run: optionalTimestamp,
      }),
    ),
  })
  .transform((raw): DataStatusView => ({
    jobs: raw.jobs.map((row): DataJobStatus => ({
      job: row.job,
      lastSuccess: row.last_success,
      lastRun: row.last_run,
      nextRun: row.next_run,
    })),
  }));

export const settleDataStatus = (
  outcome: WireOutcome,
): Result<DataStatusView, IpcFailure> =>
  settleWithSchema(outcome, dataStatusSchema);

export const getDataStatus = (): Promise<Result<DataStatusView, IpcFailure>> =>
  settleCall(commands.getDataStatus({ scope: "current" }), (outcome) =>
    settleDataStatus(wireOutcome(outcome)),
  );
