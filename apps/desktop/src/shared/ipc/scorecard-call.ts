import type { Result } from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import type { ScorecardScreenView } from "./estimate-views";
import type { IpcFailure } from "./schema";
import { settleScorecard } from "./scorecard-schema";

export const getScorecard = (): Promise<
  Result<ScorecardScreenView, IpcFailure>
> => settleCall(commands.getScorecard({ scope: "all" }), settleScorecard);
