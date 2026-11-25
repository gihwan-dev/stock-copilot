import type { Result } from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
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

export const checkTossConnection = (): Promise<
  Result<TossConnection, IpcFailure>
> =>
  settleCall(
    commands.checkTossConnection({ query: "current" }),
    settleTossConnection,
  );

export const submitTossKeys = (
  keys: TossKeys,
): Promise<Result<TossConnection, IpcFailure>> =>
  settleCall(
    commands.submitTossKeys({ client_id: keys.clientId, secret: keys.secret }),
    settleTossConnection,
  );

export const publicIp = (): Promise<Result<PublicIp, IpcFailure>> =>
  settleCall(commands.getPublicIp({ query: "current" }), settlePublicIp);

export const copyText = (
  value: IpAddressText,
): Promise<Result<CopyResult, IpcFailure>> =>
  settleCall(commands.copyText({ text: value }), settleCopyResult);
