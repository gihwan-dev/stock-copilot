import { err, type ExternalUrl, type Result } from "../lib";
import { commands } from "./bindings";
import {
  settleAppVersion,
  settleOpenExternal,
  type AppVersion,
  type IpcFailure,
  type OpenExternalResult,
} from "./schema";
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
    .catch((): Result<AppVersion, IpcFailure> => {
      timer.cancel();
      return err(frameworkFailure);
    });
  return Promise.race([request, timer.promise]);
};

export const openExternal = (
  url: ExternalUrl,
): Promise<Result<OpenExternalResult, IpcFailure>> => {
  const timer = createTimeout<OpenExternalResult>();
  const request = commands
    .openExternal({ url })
    .then((outcome): Result<OpenExternalResult, IpcFailure> => {
      timer.cancel();
      return settleOpenExternal(outcome);
    })
    .catch((): Result<OpenExternalResult, IpcFailure> => {
      timer.cancel();
      return err(frameworkFailure);
    });
  return Promise.race([request, timer.promise]);
};
