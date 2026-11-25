import type { EntityId, InputValue, Result } from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import type { SignalsView } from "./estimate-views";
import { wireOutcome, type IpcFailure } from "./schema";
import { settleApproval, settleSignals, settleSkip } from "./signals-schema";

export const getSignals = (): Promise<Result<SignalsView, IpcFailure>> =>
  settleCall(commands.getSignals({ scope: "current" }), (outcome) =>
    settleSignals(wireOutcome(outcome)),
  );

export const skipSignal = (
  card: EntityId,
  reason: InputValue,
): Promise<Result<"recorded", IpcFailure>> =>
  settleCall(commands.skipSignal({ symbol: card, reason }), (outcome) =>
    settleSkip(wireOutcome(outcome)),
  );

export const requestOrderApproval = (
  proposal: EntityId,
): Promise<Result<"requested", IpcFailure>> =>
  settleCall(
    commands.requestOrderApproval({ proposal_id: proposal }),
    (outcome) => settleApproval(wireOutcome(outcome)),
  );
