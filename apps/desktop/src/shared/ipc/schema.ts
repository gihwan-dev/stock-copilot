import { z } from "zod";

type Brand<N extends string> = Readonly<z.core.$brand<N>>;

export type AppVersionText = string & Brand<"AppVersionText">;

export type AppVersion = { readonly version: AppVersionText };

export type CommandErrorKind = "user-fixable" | "retryable" | "bug";

export type CommandError = { readonly kind: CommandErrorKind };

export const appVersionSchema = z
  .object({
    version: z
      .string()
      .regex(/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/u)
      .brand<"AppVersionText">(),
  })
  .strict()
  .readonly();

export const commandErrorSchema = z
  .object({ kind: z.enum(["user-fixable", "retryable", "bug"]) })
  .strict()
  .readonly();
