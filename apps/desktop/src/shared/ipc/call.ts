import { err, ok, type Result } from "../lib";
import { commands, events } from "./bindings";
import {
  parseUiEvent,
  settleAppVersion,
  type AppVersion,
  type CoreUiEvent,
  type IpcFailure,
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

export const listenUiEvent = (
  onEvent: (event: CoreUiEvent) => void,
): Promise<Result<() => void, IpcFailure>> =>
  events.uiEvent
    .listen((message): void => {
      const parsed = parseUiEvent(message.payload);
      if (parsed.kind === "ok") onEvent(parsed.value);
    })
    .then(ok, (): Result<() => void, IpcFailure> => err(frameworkFailure));
