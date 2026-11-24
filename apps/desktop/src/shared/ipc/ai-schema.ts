import { z } from "zod";
import {
  clockTimeSchema,
  configPathSchema,
  usagePercentSchema,
} from "./schema";

const aiTasks = [
  "chat",
  "estimate",
  "deep-research",
  "briefing",
  "news-summary",
  "collection",
] as const;
export type AiTask =
  | "chat"
  | "estimate"
  | "deep-research"
  | "briefing"
  | "news-summary"
  | "collection";
export const aiModels = ["opus", "sonnet", "haiku"] as const;
export type AiModel = "opus" | "sonnet" | "haiku";
export const aiEfforts = ["low", "medium", "high", "xhigh", "max"] as const;
export type AiEffort = "low" | "medium" | "high" | "xhigh" | "max";
export const budgetPercents = ["50", "60", "70", "80", "90"] as const;
export type BudgetPercent = "50" | "60" | "70" | "80" | "90";

const taskSettingSchema = z
  .object({
    task: z.enum(aiTasks),
    model: z.enum(aiModels),
    effort: z.enum(aiEfforts),
  })
  .strict()
  .readonly();
export type AiTaskSetting = z.infer<typeof taskSettingSchema>;
export const aiSettingsSchema = z
  .object({
    tasks: z.array(taskSettingSchema).readonly(),
    budget_percent: z.enum(budgetPercents),
    usage_percent: usagePercentSchema,
    claude_config_dir: configPathSchema.nullable(),
  })
  .strict()
  .readonly();
export type AiSettings = z.infer<typeof aiSettingsSchema>;

const scheduledJobs = [
  "morning-briefing",
  "market-watch",
  "closing-review",
  "us-premarket",
  "estimate-rotation",
  "discovery",
  "weekly-review",
] as const;
export type ScheduledJob =
  | "morning-briefing"
  | "market-watch"
  | "closing-review"
  | "us-premarket"
  | "estimate-rotation"
  | "discovery"
  | "weekly-review";
export type Toggle = "on" | "off";
export const toggleSchema = z.enum(["on", "off"]);
const jobSchema = z
  .object({
    job: z.enum(scheduledJobs),
    state: toggleSchema,
    time: clockTimeSchema.nullable(),
  })
  .strict()
  .readonly();
export type ScheduledJobSetting = z.infer<typeof jobSchema>;
export const jobsSchema = z
  .object({ jobs: z.array(jobSchema).readonly() })
  .strict()
  .readonly();
export type ScheduledJobs = z.infer<typeof jobsSchema>;
