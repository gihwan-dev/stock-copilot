import { err, ok, type Result } from "../lib";
import { commands } from "./bindings";
import {
  appVersionSchema,
  commandErrorSchema,
  type AppVersion,
  type CommandError,
} from "./schema";

export type IpcFailure =
  | { readonly kind: "command"; readonly error: CommandError }
  | { readonly kind: "framework" }
  | { readonly kind: "invalid-response" };

const frameworkFailure: IpcFailure = { kind: "framework" };

export const appVersion = (): Promise<Result<AppVersion, IpcFailure>> =>
  commands
    .appVersion()
    .then((outcome): Result<AppVersion, IpcFailure> => {
      switch (outcome.status) {
        case "ok": {
          const parsed = appVersionSchema.safeParse(outcome.data);
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
    })
    .catch((): Result<AppVersion, IpcFailure> => err(frameworkFailure));
