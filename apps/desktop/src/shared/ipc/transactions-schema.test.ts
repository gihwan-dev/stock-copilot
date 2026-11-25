import { expect, test } from "vitest";
import { text } from "../lib";
import { wire } from "./schema";
import { responses } from "./testing/responses";
import { settleTransactions } from "./transactions-schema";

const recorded = responses.getTransactions;

test("settleTransactions keeps the recorded rows, names cash rows and marks vanished rows", () => {
  const result = settleTransactions({ status: "ok", data: wire(recorded) });
  expect(result.kind).toBe("ok");
  if (result.kind !== "ok") return;
  const rows = result.value.rows;
  expect(rows.map((row) => row.kind)).toStrictEqual([
    "buy",
    "sell",
    "deposit",
    "other",
  ]);
  expect(rows[0]?.name).toBe(recorded.rows[0].name);
  expect(rows[0]?.quantity === undefined).toBe(false);
  expect(rows[1]?.presence).toBe("vanished");
  expect(rows[2]?.name).toBe(text("cash.KRW", "ko"));
  expect(rows[2]?.quantity === undefined).toBe(true);
});

test("settleTransactions refuses a row with an unknown kind or extra field", () => {
  const row = recorded.rows[0];
  const results = [
    { ...row, kind: "loan" },
    { ...row, account: "413" },
  ].map(
    (changed) =>
      settleTransactions({
        status: "ok",
        data: wire({ ...recorded, rows: [changed] }),
      }).kind,
  );
  expect(results).toStrictEqual(["err", "err"]);
});

test("settleTransactions passes a command error through", () => {
  const result = settleTransactions({
    status: "error",
    error: wire({ kind: "no-data" }),
  });
  expect(result).toStrictEqual({
    kind: "err",
    error: { kind: "command", error: { kind: "no-data" } },
  });
});
