import { z } from "zod";
import { freeTextSchema, type FreeText, type Result } from "../lib";
import { settleWithSchema, type IpcFailure, type WireOutcome } from "./schema";

export type UpdateStatusQuery = "current" | "check-now";
export type UpdateRequest = UpdateStatusQuery | "restart";

export type UpdateFailureKind = "check" | "download" | "install";

export type UpdateState =
  | { readonly kind: "off" }
  | { readonly kind: "idle" }
  | { readonly kind: "checking" }
  | { readonly kind: "downloading"; readonly version: FreeText }
  | { readonly kind: "up-to-date" }
  | {
      readonly kind: "ready";
      readonly version: FreeText;
      readonly notes: FreeText;
    }
  | { readonly kind: "failed"; readonly failure: UpdateFailureKind };

export type UpdateStatus = {
  readonly currentVersion: FreeText;
  readonly state: UpdateState;
};

const versionSchema = z
  .string()
  .regex(/^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/u)
  .pipe(freeTextSchema);

const plainState = z
  .enum(["off", "idle", "checking", "up-to-date"])
  .transform((kind): UpdateState => ({ kind }));

const downloadingState = z
  .object({
    downloading: z.object({ version: versionSchema }).strict(),
  })
  .strict()
  .transform((wire): UpdateState => ({
    kind: "downloading",
    version: wire.downloading.version,
  }));

const readyState = z
  .object({
    ready: z
      .object({
        version: versionSchema,
        notes: z.string().pipe(freeTextSchema),
      })
      .strict(),
  })
  .strict()
  .transform((wire): UpdateState => ({
    kind: "ready",
    version: wire.ready.version,
    notes: wire.ready.notes,
  }));

const failedState = z
  .object({
    failed: z
      .object({ failure: z.enum(["check", "download", "install"]) })
      .strict(),
  })
  .strict()
  .transform((wire): UpdateState => ({
    kind: "failed",
    failure: wire.failed.failure,
  }));

const updateStatusSchema = z
  .object({
    current_version: versionSchema,
    state: z.union([plainState, downloadingState, readyState, failedState]),
  })
  .strict()
  .transform((wire): UpdateStatus => ({
    currentVersion: wire.current_version,
    state: wire.state,
  }));

export const settleUpdateStatus = (
  outcome: WireOutcome,
): Result<UpdateStatus, IpcFailure> =>
  settleWithSchema(outcome, updateStatusSchema);
