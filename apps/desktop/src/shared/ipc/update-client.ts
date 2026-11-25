import type { Result } from "../lib";
import { updateCommand } from "./call";
import type { IpcFailure } from "./schema";
import type { UpdateStatus, UpdateStatusQuery } from "./update-schema";

export type UpdateClient = {
  readonly status: (
    query: UpdateStatusQuery,
  ) => Promise<Result<UpdateStatus, IpcFailure>>;
  readonly restart: () => Promise<Result<UpdateStatus, IpcFailure>>;
};

export const coreUpdateClient: UpdateClient = {
  status: updateCommand,
  restart: () => updateCommand("restart"),
};
