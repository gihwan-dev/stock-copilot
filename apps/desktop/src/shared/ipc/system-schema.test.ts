import { expect, test } from "vitest";
import {
  accountsSchema,
  backupSchema,
  deleteSchema,
  exportSchema,
  principlesSchema,
} from "./setup-schema";
import { responses } from "./testing/responses";

test("recorded system replies match the settings screen schemas", () => {
  expect(accountsSchema.safeParse(responses.getAccountSelection).success).toBe(
    true,
  );
  expect(accountsSchema.safeParse(responses.selectAccount).success).toBe(true);
  expect(principlesSchema.safeParse(responses.getPrinciples).success).toBe(
    true,
  );
  expect(principlesSchema.safeParse(responses.setPrinciples).success).toBe(
    true,
  );
  expect(backupSchema.safeParse(responses.backupNow).success).toBe(true);
  expect(exportSchema.safeParse(responses.exportData).success).toBe(true);
  expect(deleteSchema.safeParse(responses.deleteAllData).success).toBe(true);
});

test("an account reply without a selection is refused", () => {
  expect(
    accountsSchema.safeParse({ ...responses.getAccountSelection, selected: "" })
      .success,
  ).toBe(false);
});
