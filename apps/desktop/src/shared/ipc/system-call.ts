import type { Result } from "../lib";
import { commands } from "./bindings";
import { settleCall } from "./call";
import { settleWithSchema, type AccountRef, type IpcFailure } from "./schema";
import {
  accountsSchema,
  backupSchema,
  deleteSchema,
  exportSchema,
  principlesSchema,
  type AccountSelection,
  type BackupResult,
  type DeleteResult,
  type ExportResult,
  type Principles,
} from "./setup-schema";

type Reply<T> = Promise<Result<T, IpcFailure>>;

export const accountSelection = (): Reply<AccountSelection> =>
  settleCall(commands.getAccountSelection({ scope: "current" }), (outcome) =>
    settleWithSchema(outcome, accountsSchema),
  );

export const selectAccount = (id: AccountRef): Reply<AccountSelection> =>
  settleCall(commands.selectAccount({ account: id }), (outcome) =>
    settleWithSchema(outcome, accountsSchema),
  );

export const backupNow = (): Reply<BackupResult> =>
  settleCall(commands.backupNow({ scope: "current" }), (outcome) =>
    settleWithSchema(outcome, backupSchema),
  );

export const exportData = (): Reply<ExportResult> =>
  settleCall(commands.exportData({ scope: "current" }), (outcome) =>
    settleWithSchema(outcome, exportSchema),
  );

export const deleteAllData = (): Reply<DeleteResult> =>
  settleCall(commands.deleteAllData({ scope: "current" }), (outcome) =>
    settleWithSchema(outcome, deleteSchema),
  );

export const principles = (): Reply<Principles> =>
  settleCall(commands.getPrinciples({ scope: "current" }), (outcome) =>
    settleWithSchema(outcome, principlesSchema),
  );

export const setPrinciples = (value: Principles): Reply<Principles> =>
  settleCall(
    commands.setPrinciples({
      principles: {
        text: value.text,
        cash_min_percent: value.cash_min_percent,
        concentration_min_count: value.concentration_min_count,
      },
    }),
    (outcome) => settleWithSchema(outcome, principlesSchema),
  );
