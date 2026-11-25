import { err, type Result } from "../lib";
import { commands } from "./bindings";
import { frameworkFailure, raceTimeout } from "./call";
import type { IpAddressText, IpcFailure, SecretText } from "./schema";
import {
  settleCopyResult,
  settlePublicIp,
  settleTossConnection,
} from "./settings-schema";
import type { CopyResult, PublicIp, TossConnection } from "./setup-schema";

export type TossKeys = {
  readonly clientId: SecretText;
  readonly secret: SecretText;
};

const failed = (): Result<TossConnection, IpcFailure> => err(frameworkFailure);

export const checkTossConnection = (): Promise<
  Result<TossConnection, IpcFailure>
> =>
  raceTimeout(
    commands
      .checkTossConnection({ query: "current" })
      .then(settleTossConnection)
      .catch(failed),
  );

export const submitTossKeys = (
  keys: TossKeys,
): Promise<Result<TossConnection, IpcFailure>> =>
  raceTimeout(
    commands
      .submitTossKeys({ client_id: keys.clientId, secret: keys.secret })
      .then(settleTossConnection)
      .catch(failed),
  );

export const publicIp = (): Promise<Result<PublicIp, IpcFailure>> =>
  raceTimeout(
    commands
      .getPublicIp({ query: "current" })
      .then(settlePublicIp)
      .catch((): Result<PublicIp, IpcFailure> => err(frameworkFailure)),
  );

export const copyText = (
  value: IpAddressText,
): Promise<Result<CopyResult, IpcFailure>> =>
  raceTimeout(
    commands
      .copyText({ text: value })
      .then(settleCopyResult)
      .catch((): Result<CopyResult, IpcFailure> => err(frameworkFailure)),
  );
