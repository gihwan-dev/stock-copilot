import { z } from "zod";
import { err, ok, type Result } from "../lib";

type Brand<N extends string> = Readonly<z.core.$brand<N>>;

type AppVersionText = string & Brand<"AppVersionText">;

export type AppVersion = { readonly version: AppVersionText };

type CommandErrorKind = "user-fixable" | "retryable" | "bug" | "no-data";

export type CommandError = { readonly kind: CommandErrorKind };

export type IpcFailure =
  | { readonly kind: "command"; readonly error: CommandError }
  | { readonly kind: "framework" }
  | { readonly kind: "invalid-response" }
  | { readonly kind: "timeout" };

type CommandOutcome =
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

export type OpenExternalStatus = "opened";

export type OpenExternalResult = { readonly status: OpenExternalStatus };

export const openExternalResultSchema = z
  .object({ status: z.literal("opened") })
  .strict()
  .readonly();
const settleWithSchema = <T>(
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
