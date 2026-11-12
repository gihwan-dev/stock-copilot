import { expect, it } from "vitest";

const rows: readonly (readonly ["a"])[] = [];

it.each(rows)("row %s", () => {
  expect(1).toBe(1);
});
