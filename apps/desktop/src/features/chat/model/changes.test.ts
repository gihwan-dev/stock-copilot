import { expect, test } from "vitest";
import { withChange } from "./changes";

test("only finished AI changes become chat notices, once each", () => {
  expect(withChange([], "change-principles", "running")).toStrictEqual([]);
  expect(withChange([], "change-principles", "failed")).toStrictEqual([]);
  expect(withChange([], "portfolio", "succeeded")).toStrictEqual([]);
  expect(withChange([], "app-data", "succeeded")).toStrictEqual([]);
  expect(withChange([], "change-principles", "succeeded")).toStrictEqual([
    "change-principles",
  ]);
  expect(
    withChange(["change-principles"], "change-principles", "succeeded"),
  ).toStrictEqual(["change-principles"]);
  expect(
    withChange(["change-principles"], "approval-request", "succeeded"),
  ).toStrictEqual(["change-principles", "approval-request"]);
});
