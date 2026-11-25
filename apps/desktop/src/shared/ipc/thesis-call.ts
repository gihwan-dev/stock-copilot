import type { EntityId, Result } from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import type { ThesisView } from "./estimate-views";
import { wireOutcome, type IpcFailure } from "./schema";
import { settleThesis } from "./thesis-schema";

export const getThesis = (
  instrument: EntityId,
): Promise<Result<ThesisView, IpcFailure>> =>
  settleCall(commands.getThesis({ symbol: instrument }), (outcome) =>
    settleThesis(wireOutcome(outcome)),
  );
