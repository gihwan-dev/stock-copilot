import { z } from "zod";
import type { Result } from "../lib";
import {
  jobsSchema,
  type ScheduledJob,
  type ScheduledJobSetting,
  type ScheduledJobs,
} from "./ai-schema";
import { commands } from "./bindings";
import { settleCall } from "./call";
import {
  settleWithSchema,
  wireOutcome,
  type IpcFailure,
  type WireOutcome,
} from "./schema";

type Reply<T> = Promise<Result<T, IpcFailure>>;

const wireJobs = {
  "morning-briefing": "morning-briefing",
  "intraday-watch": "market-watch",
  "closing-review": "closing-review",
  "pre-us-market-check": "us-premarket",
  "estimate-cycle": "estimate-rotation",
  "candidate-discovery": "discovery",
  "weekly-review": "weekly-review",
  "news-collection": undefined,
} as const;

const screenJobs = {
  "morning-briefing": "morning-briefing",
  "market-watch": "intraday-watch",
  "closing-review": "closing-review",
  "us-premarket": "pre-us-market-check",
  "estimate-rotation": "estimate-cycle",
  discovery: "candidate-discovery",
  "weekly-review": "weekly-review",
} as const satisfies Record<ScheduledJob, keyof typeof wireJobs>;

const wireSchema = z
  .object({
    jobs: z.array(
      z
        .object({
          job: z.enum([
            "morning-briefing",
            "intraday-watch",
            "closing-review",
            "pre-us-market-check",
            "estimate-cycle",
            "candidate-discovery",
            "weekly-review",
            "news-collection",
          ]),
          enabled: z.enum(["on", "off"]),
          time: z
            .object({ hour: z.number().int(), minute: z.number().int() })
            .nullable(),
        })
        .loose(),
    ),
  })
  .loose()
  .transform((raw, context): ScheduledJobs => {
    const converted = {
      jobs: raw.jobs.flatMap((row) => {
        const job = wireJobs[row.job];
        return job === undefined
          ? []
          : [
              {
                job,
                state: row.enabled,
                time:
                  row.time === null
                    ? null
                    : `${String(row.time.hour).padStart(2, "0")}:${String(row.time.minute).padStart(2, "0")}`,
              },
            ];
      }),
    };
    const parsed = jobsSchema.safeParse(converted);
    if (parsed.success) return parsed.data;
    context.addIssue({ code: "custom", message: "jobs" });
    return z.NEVER;
  });

export const settleSchedules = (
  outcome: WireOutcome,
): Result<ScheduledJobs, IpcFailure> => settleWithSchema(outcome, wireSchema);

const kstTime = z
  .string()
  .regex(/^\d{2}:\d{2}$/u)
  .transform((value) => {
    const [hour, minute] = value.split(":");
    return { hour: Number(hour), minute: Number(minute) };
  });

export const scheduledJobs = (): Reply<ScheduledJobs> =>
  settleCall(commands.getJobSchedules({ scope: "ai" }), (outcome) =>
    settleSchedules(wireOutcome(outcome)),
  );

export const setScheduledJob = (
  setting: ScheduledJobSetting,
): Reply<ScheduledJobs> => {
  const time =
    setting.time === null ? undefined : kstTime.safeParse(setting.time);
  return settleCall(
    commands.setJobSchedule({
      job: screenJobs[setting.job],
      enabled: setting.state,
      time: time?.success === true ? time.data : null,
    }),
    (outcome) => settleSchedules(wireOutcome(outcome)),
  );
};
