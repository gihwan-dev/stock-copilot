import { z } from "zod";
import { err, ok, parseTicker, type Result, type Ticker } from "../lib";

type Brand<N extends string> = Readonly<z.core.$brand<N>>;

export type AppVersionText = string & Brand<"AppVersionText">;

export type AppVersion = { readonly version: AppVersionText };

export type CommandErrorKind = "user-fixable" | "retryable" | "bug" | "no-data";

export type CommandError = { readonly kind: CommandErrorKind };

export type IpcFailure =
  | { readonly kind: "command"; readonly error: CommandError }
  | { readonly kind: "framework" }
  | { readonly kind: "invalid-response" }
  | { readonly kind: "timeout" };

export type CommandOutcome =
  | { readonly status: "ok"; readonly data: unknown }
  | { readonly status: "error"; readonly error: unknown };

const frameworkFailure: IpcFailure = { kind: "framework" };

const appVersionSchema = z
  .object({
    version: z
      .string()
      .regex(/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/u)
      .brand<"AppVersionText">(),
  })
  .strict()
  .readonly();

const commandErrorSchema = z
  .object({ kind: z.enum(["user-fixable", "retryable", "bug", "no-data"]) })
  .strict()
  .readonly();

type OpenExternalStatus = "opened";

export type OpenExternalResult = { readonly status: OpenExternalStatus };

const openExternalResultSchema = z
  .object({ status: z.literal("opened") })
  .strict()
  .readonly();

export const settleWithSchema = <T>(
  outcome: CommandOutcome,
  schema: Readonly<z.ZodType<T>>,
): Result<T, IpcFailure> => {
  switch (outcome.status) {
    case "ok": {
      const parsed = schema.safeParse(outcome.data);
      return parsed.success
        ? ok(parsed.data)
        : err({ kind: "invalid-response" });
    }
    case "error": {
      const parsed = commandErrorSchema.safeParse(outcome.error);
      return parsed.success
        ? err({ kind: "command", error: parsed.data })
        : err(frameworkFailure);
    }
  }
};

export const settleAppVersion = (
  outcome: CommandOutcome,
): Result<AppVersion, IpcFailure> =>
  settleWithSchema(outcome, appVersionSchema);

export const settleOpenExternal = (
  outcome: CommandOutcome,
): Result<OpenExternalResult, IpcFailure> =>
  settleWithSchema(outcome, openExternalResultSchema);

export type Wire = Brand<"Wire">;

export type WireOutcome =
  | { readonly status: "ok"; readonly data: Wire }
  | { readonly status: "error"; readonly error: Wire };

export const wire = (raw: unknown): Wire => raw as Wire;

export const wireOutcome = (outcome: CommandOutcome): WireOutcome =>
  outcome.status === "ok"
    ? { status: "ok", data: wire(outcome.data) }
    : { status: "error", error: wire(outcome.error) };
export type SecretText = string & Brand<"SecretText">;
export type IpAddressText = string & Brand<"IpAddressText">;
export type AccountRef = string & Brand<"AccountRef">;
export type AccountNumberText = string & Brand<"AccountNumberText">;
export type ClockTime = string & Brand<"ClockTime">;
export type ConfigPathText = string & Brand<"ConfigPathText">;
export type ContactText = string & Brand<"ContactText">;
export type AliasText = string & Brand<"AliasText">;
export type PrincipleText = string & Brand<"PrincipleText">;
export type UsagePercent = string & Brand<"UsagePercent">;

export type FieldError = { readonly kind: "invalid-field" };

export const secretSchema = z
  .string()
  .trim()
  .min(1)
  .max(512)
  .regex(/^\S+$/u)
  .brand<"SecretText">();
export const ipSchema = z.union([z.ipv4(), z.ipv6()]).brand<"IpAddressText">();
export const accountRefSchema = z.string().min(1).max(64).brand<"AccountRef">();
export const accountNumberSchema = z
  .string()
  .min(1)
  .max(64)
  .brand<"AccountNumberText">();
export const clockTimeSchema = z
  .string()
  .regex(/^(?:[01][0-9]|2[0-3]):[0-5][0-9]$/u)
  .brand<"ClockTime">();
export const configPathSchema = z
  .string()
  .trim()
  .min(1)
  .max(1024)
  .regex(/^[/~]/u)
  .brand<"ConfigPathText">();
export const contactSchema = z.email().max(254).brand<"ContactText">();
export const aliasSchema = z
  .string()
  .trim()
  .min(1)
  .max(40)
  .brand<"AliasText">();
export const principleSchema = z.string().max(4000).brand<"PrincipleText">();
export const usagePercentSchema = z
  .string()
  .regex(/^(?:100|[1-9]?[0-9])(?:\.[0-9]{1,2})?$/u)
  .brand<"UsagePercent">();
export const tickerSchema = z.string().transform((input, context): Ticker => {
  const parsed = parseTicker(input);
  if (parsed.kind === "ok") return parsed.value;
  context.addIssue({ code: "custom", message: "ticker" });
  return z.NEVER;
});

const fromField = <T>(
  parsed: Readonly<z.ZodSafeParseResult<T>>,
): Result<T, FieldError> =>
  parsed.success ? ok(parsed.data) : err({ kind: "invalid-field" });

export const parseSecretText = (
  input: unknown,
): Result<SecretText, FieldError> => fromField(secretSchema.safeParse(input));
export const parseClockTime = (input: unknown): Result<ClockTime, FieldError> =>
  fromField(clockTimeSchema.safeParse(input));
export const parseConfigPath = (
  input: unknown,
): Result<ConfigPathText, FieldError> =>
  fromField(configPathSchema.safeParse(input));
export const parseContact = (input: unknown): Result<ContactText, FieldError> =>
  fromField(contactSchema.safeParse(input));
export const parseAlias = (input: unknown): Result<AliasText, FieldError> =>
  fromField(aliasSchema.safeParse(input));
export const parsePrinciple = (
  input: unknown,
): Result<PrincipleText, FieldError> =>
  fromField(principleSchema.safeParse(input));
export const parseTickerField = (input: unknown): Result<Ticker, FieldError> =>
  fromField(tickerSchema.safeParse(input));
