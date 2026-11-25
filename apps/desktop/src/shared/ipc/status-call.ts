import type { Result } from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import type { IpcFailure } from "./schema";
import type { SelfCheckView, StatusView } from "./screen-sources";
import { settleSelfCheck, settleStatus } from "./status-schema";

export const getSelfCheck = (): Promise<Result<SelfCheckView, IpcFailure>> =>
  settleCall(commands.getSelfCheck({ scope: "current" }), settleSelfCheck);

export const getStatus = (): Promise<Result<StatusView, IpcFailure>> =>
  settleCall(commands.getStatus({ scope: "current" }), settleStatus);
