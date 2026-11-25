import { z } from "zod";
import { freeTextSchema, parseEntityId, text, type Result } from "../lib";
import { settleWithSchema, type IpcFailure, type WireOutcome } from "./schema";
import type { TransactionRow, TransactionsView } from "./screen-sources";
import {
  uiFreeText,
  wireMoney,
  wireQuantity,
  wireTimestamp,
} from "./wire-values";

const cashKeys = { KRW: "cash.KRW", USD: "cash.USD" } as const;

const rowSchema = z
  .strictObject({
    id: z.string(),
    settled_at: wireTimestamp,
    symbol: z.union([z.string(), z.null()]),
    name: z.union([freeTextSchema, z.null()]),
    cash: z.union([z.enum(["KRW", "USD"]), z.null()]),
    kind: z.enum([
      "buy",
      "sell",
      "dividend",
      "deposit",
      "withdrawal",
      "fee",
      "other",
    ]),
    quantity: z.union([wireQuantity, z.null()]),
    amount: wireMoney,
    presence: z.enum(["present", "vanished"]),
  })
  .transform((raw, context): TransactionRow => {
    const id = parseEntityId(raw.id);
    if (id.kind === "err") {
      context.addIssue({ code: "custom", message: "id" });
      return z.NEVER;
    }
    const name =
      raw.name ??
      uiFreeText(text(cashKeys[raw.cash ?? raw.amount.currency], "ko"));
    return {
      id: id.value,
      settledAt: raw.settled_at,
      name,
      kind: raw.kind,
      quantity: raw.quantity ?? undefined,
      amount: raw.amount,
      presence: raw.presence,
    };
  });

const viewSchema = z
  .strictObject({ recorded_at: wireTimestamp, rows: z.array(rowSchema) })
  .transform((raw): TransactionsView => ({
    recordedAt: raw.recorded_at,
    rows: raw.rows,
  }));

export const settleTransactions = (
  outcome: WireOutcome,
): Result<TransactionsView, IpcFailure> =>
  settleWithSchema(outcome, viewSchema);
