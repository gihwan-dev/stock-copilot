import type { Result } from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import { settleAllocation, settlePortfolio } from "./portfolio-schema";
import { wireOutcome, type IpcFailure } from "./schema";
import type { AllocationView, PortfolioView } from "./screen-sources";

export const getPortfolio = (): Promise<Result<PortfolioView, IpcFailure>> =>
  settleCall(commands.getPortfolio({ scope: "current" }), (outcome) =>
    settlePortfolio(wireOutcome(outcome)),
  );

export const getAllocation = (): Promise<Result<AllocationView, IpcFailure>> =>
  settleCall(commands.getAllocation({ scope: "current" }), (outcome) =>
    settleAllocation(wireOutcome(outcome)),
  );
