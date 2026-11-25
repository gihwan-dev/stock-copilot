import type { Result } from "../lib";
import {
  aiSettingsSchema,
  type AiSettings,
  type AiTaskSetting,
  type BudgetPercent,
} from "./ai-schema";
import { commands } from "./bindings";
import { settleCall } from "./call";
import {
  settleWithSchema,
  type ConfigPathText,
  type IpcFailure,
} from "./schema";

type Reply<T> = Promise<Result<T, IpcFailure>>;

export const aiSettings = (): Reply<AiSettings> =>
  settleCall(commands.getAiSettings({ scope: "current" }), (outcome) =>
    settleWithSchema(outcome, aiSettingsSchema),
  );

export const setTaskModel = (setting: AiTaskSetting): Reply<AiSettings> =>
  settleCall(
    commands.setTaskModel({
      setting: {
        task: setting.task,
        model: setting.model,
        effort: setting.effort,
      },
    }),
    (outcome) => settleWithSchema(outcome, aiSettingsSchema),
  );

export const setBudgetPercent = (percent: BudgetPercent): Reply<AiSettings> =>
  settleCall(commands.setBudgetPercent({ percent }), (outcome) =>
    settleWithSchema(outcome, aiSettingsSchema),
  );

export const setClaudeConfigDir = (
  path: ConfigPathText | null,
): Reply<AiSettings> =>
  settleCall(commands.setClaudeConfigDir({ path }), (outcome) =>
    settleWithSchema(outcome, aiSettingsSchema),
  );
