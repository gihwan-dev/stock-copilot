import { describe, expect, it } from "vitest";

it("passes", () => {
  expect(1).toBe(1);
});

it.only("focused", () => {
  expect(1).toBe(1);
});
