import { z } from "zod";
import {
  accountNumberSchema,
  accountRefSchema,
  ipSchema,
  principleSchema,
} from "./schema";

const connectionStates = [
  "needs-keys",
  "connected",
  "ip-not-allowed",
  "invalid-key",
  "forbidden",
  "account-not-found",
  "key-in-use",
] as const;
export type ConnectionState =
  | "needs-keys"
  | "connected"
  | "ip-not-allowed"
  | "invalid-key"
  | "forbidden"
  | "account-not-found"
  | "key-in-use";
export const connectionSchema = z
  .object({ state: z.enum(connectionStates) })
  .strict()
  .readonly();
export type TossConnection = z.infer<typeof connectionSchema>;

export const publicIpSchema = z
  .object({ address: ipSchema })
  .strict()
  .readonly();
export type PublicIp = z.infer<typeof publicIpSchema>;

export const copiedSchema = z
  .object({ status: z.literal("copied") })
  .strict()
  .readonly();
export type CopyResult = z.infer<typeof copiedSchema>;

export const accountsSchema = z
  .object({
    selected: accountRefSchema,
    accounts: z
      .array(
        z
          .object({ id: accountRefSchema, number: accountNumberSchema })
          .strict()
          .readonly(),
      )
      .readonly(),
  })
  .strict()
  .readonly();
export type AccountSelection = z.infer<typeof accountsSchema>;

export const backupSchema = z
  .object({ status: z.literal("backed-up") })
  .strict()
  .readonly();
export type BackupResult = z.infer<typeof backupSchema>;
export const exportSchema = z
  .object({ status: z.enum(["exported", "cancelled"]) })
  .strict()
  .readonly();
export type ExportResult = z.infer<typeof exportSchema>;
export const deleteSchema = z
  .object({ status: z.literal("deleted") })
  .strict()
  .readonly();
export type DeleteResult = z.infer<typeof deleteSchema>;

export const cashMinPercents = ["0", "5", "10", "20", "30"] as const;
export type CashMinPercent = "0" | "5" | "10" | "20" | "30";
export const concentrationCounts = ["3", "4", "5", "6", "8", "10"] as const;
export type ConcentrationCount = "3" | "4" | "5" | "6" | "8" | "10";
export const principlesSchema = z
  .object({
    text: principleSchema,
    cash_min_percent: z.enum(cashMinPercents),
    concentration_min_count: z.enum(concentrationCounts),
  })
  .strict()
  .readonly();
export type Principles = z.infer<typeof principlesSchema>;
