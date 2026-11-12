import { expect, it } from "vitest";

it.each(["a", "b"])("row %s", (row) => {
  expect(row).toBeTypeOf("string");
});
