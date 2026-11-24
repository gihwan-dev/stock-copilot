import { z } from "zod";
import { toggleSchema } from "./ai-schema";
import { aliasSchema, contactSchema, tickerSchema } from "./schema";

export const newsSources = [
  "KindDisclosure",
  "DartDisclosureApi",
  "DartDisclosureRss",
  "DartFundamentals",
  "Hankyung",
  "Maekyung",
  "Yonhap",
  "Edaily",
  "GoogleNews",
  "YahooFinance",
  "SecEdgar",
  "BankOfKorea",
  "FederalReserve",
  "UsTreasury",
  "Fred",
] as const;
export type NewsSource =
  | "KindDisclosure"
  | "DartDisclosureApi"
  | "DartDisclosureRss"
  | "DartFundamentals"
  | "Hankyung"
  | "Maekyung"
  | "Yonhap"
  | "Edaily"
  | "GoogleNews"
  | "YahooFinance"
  | "SecEdgar"
  | "BankOfKorea"
  | "FederalReserve"
  | "UsTreasury"
  | "Fred";
export const intervalMinutes = ["1", "2", "5", "10", "15", "30", "60"] as const;
export type IntervalMinutes = "1" | "2" | "5" | "10" | "15" | "30" | "60";
export type BodyReading = "on" | "off" | "locked";
export const dailyMovePercents = ["3", "5", "7", "10"] as const;
export type DailyMovePercent = "3" | "5" | "7" | "10";
export const newsPerHourCounts = ["3", "5", "10", "20"] as const;
export type NewsPerHour = "3" | "5" | "10" | "20";
export type OptionalKey = "dart" | "fred";

const sourceSchema = z
  .object({
    source: z.enum(newsSources),
    state: toggleSchema,
    body_reading: z.enum(["on", "off", "locked"]),
    interval_minutes: z.enum(intervalMinutes),
  })
  .strict()
  .readonly();
export type SourceSetting = z.infer<typeof sourceSchema>;
const aliasRowSchema = z
  .object({ alias: aliasSchema, ticker: tickerSchema })
  .strict()
  .readonly();
export type AliasRow = z.infer<typeof aliasRowSchema>;
const presenceSchema = z.enum(["set", "unset"]);
const wakeSchema = z
  .object({
    daily_move_percent: z.enum(dailyMovePercents),
    news_per_hour: z.enum(newsPerHourCounts),
  })
  .strict()
  .readonly();
export type WakeCriteria = z.infer<typeof wakeSchema>;
export const newsSettingsSchema = z
  .object({
    sources: z.array(sourceSchema).readonly(),
    keys: z
      .object({ dart: presenceSchema, fred: presenceSchema })
      .strict()
      .readonly(),
    sec_contact: contactSchema.nullable(),
    wake: wakeSchema,
    aliases: z.array(aliasRowSchema).readonly(),
  })
  .strict()
  .readonly();
export type NewsSettings = z.infer<typeof newsSettingsSchema>;
