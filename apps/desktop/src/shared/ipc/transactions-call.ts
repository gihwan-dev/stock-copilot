import type { Result } from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import { wireOutcome, type IpcFailure } from "./schema";
import type { TransactionsView } from "./screen-sources";
import { settleTransactions } from "./transactions-schema";

export const getTransactions = (): Promise<
  Result<TransactionsView, IpcFailure>
> =>
  settleCall(commands.getTransactions({ scope: "recent" }), (outcome) =>
    settleTransactions(wireOutcome(outcome)),
  );
