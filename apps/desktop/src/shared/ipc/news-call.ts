import type { Result } from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import {
  newsSettingsSchema,
  type AliasRow,
  type NewsSettings,
  type OptionalKey,
  type SourceSetting,
  type WakeCriteria,
} from "./news-schema";
import {
  settleWithSchema,
  type ContactText,
  type IpcFailure,
  type SecretText,
} from "./schema";

type Reply<T> = Promise<Result<T, IpcFailure>>;

export const newsSettings = (): Reply<NewsSettings> =>
  settleCall(commands.getNewsSettings({ scope: "current" }), (outcome) =>
    settleWithSchema(outcome, newsSettingsSchema),
  );

export const setNewsSource = (setting: SourceSetting): Reply<NewsSettings> =>
  settleCall(
    commands.setNewsSource({
      setting: {
        source: setting.source,
        state: setting.state,
        body_reading: setting.body_reading,
        interval_minutes: setting.interval_minutes,
      },
    }),
    (outcome) => settleWithSchema(outcome, newsSettingsSchema),
  );

export const setOptionalKey = (
  key: OptionalKey,
  secret: SecretText,
): Reply<NewsSettings> =>
  settleCall(commands.setOptionalKey({ key, secret }), (outcome) =>
    settleWithSchema(outcome, newsSettingsSchema),
  );

export const setSecContact = (
  contact: ContactText | null,
): Reply<NewsSettings> =>
  settleCall(commands.setSecContact({ contact }), (outcome) =>
    settleWithSchema(outcome, newsSettingsSchema),
  );

export const setWakeCriteria = (wake: WakeCriteria): Reply<NewsSettings> =>
  settleCall(
    commands.setWakeCriteria({
      wake: {
        daily_move_percent: wake.daily_move_percent,
        news_per_hour: wake.news_per_hour,
      },
    }),
    (outcome) => settleWithSchema(outcome, newsSettingsSchema),
  );

export const setAliases = (aliases: readonly AliasRow[]): Reply<NewsSettings> =>
  settleCall(
    commands.setAliases({
      aliases: aliases.map((row) => ({ alias: row.alias, ticker: row.ticker })),
    }),
    (outcome) => settleWithSchema(outcome, newsSettingsSchema),
  );
