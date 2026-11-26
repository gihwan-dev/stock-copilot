import { z } from "zod";
import type { Result } from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import {
  settleWithSchema,
  wireOutcome,
  type IpcFailure,
  type WireOutcome,
} from "./schema";
import type { ToggleState } from "./settings-schema";

const autoStartSchema = z
  .strictObject({ auto_start_at_login: z.enum(["on", "off"]) })
  .transform((raw): ToggleState => raw.auto_start_at_login);

export const settleAutoStart = (
  outcome: WireOutcome,
): Result<ToggleState, IpcFailure> =>
  settleWithSchema(outcome, autoStartSchema);

export const setAutoStart = (
  state: ToggleState,
): Promise<Result<ToggleState, IpcFailure>> =>
  settleCall(commands.setAutoStart({ state }), (outcome) =>
    settleAutoStart(wireOutcome(outcome)),
  );
