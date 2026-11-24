import { err, ok, type ExternalUrl, type Result } from "../lib";
import { commands } from "./bindings";
import {
  appVersionSchema,
  commandErrorSchema,
  openExternalResultSchema,
  type AppVersion,
  type CommandError,
  type OpenExternalResult,
} from "./schema";
import { err, type Result } from "../lib";
import { commands } from "./bindings";
import { settleAppVersion, type AppVersion, type IpcFailure } from "./schema";
import { startIpcTimeout } from "./timer";

export type { IpcFailure } from "./schema";

const frameworkFailure: IpcFailure = { kind: "framework" };
const timeoutFailure: IpcFailure = { kind: "timeout" };

const createTimeout = <T>(): {
  readonly promise: Promise<Result<T, IpcFailure>>;
  readonly cancel: () => void;
} => {
  const deferred = Promise.withResolvers<Result<T, IpcFailure>>();
  const cancel = startIpcTimeout((): void => {
    deferred.resolve(err(timeoutFailure));
  });
  return { promise: deferred.promise, cancel };
};

export const appVersion = (): Promise<Result<AppVersion, IpcFailure>> => {
  const timer = createTimeout<AppVersion>();
  const request = commands
    .appVersion({ query: "current" })
    .then((outcome): Result<AppVersion, IpcFailure> => {
      timer.cancel();
      return settleAppVersion(outcome);
    })
    .catch((): Result<AppVersion, IpcFailure> => err(frameworkFailure));

export const openExternal = (
  url: ExternalUrl,
): Promise<Result<OpenExternalResult, IpcFailure>> =>
  commands
    .openExternal({ url })
    .then((outcome): Result<OpenExternalResult, IpcFailure> => {
      switch (outcome.status) {
        case "ok": {
          const parsed = openExternalResultSchema.safeParse(outcome.data);
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
    .catch((): Result<OpenExternalResult, IpcFailure> => err(frameworkFailure));
    .catch((): Result<AppVersion, IpcFailure> => {
      timer.cancel();
      return err(frameworkFailure);
    });
  return Promise.race([request, timer.promise]);
};
