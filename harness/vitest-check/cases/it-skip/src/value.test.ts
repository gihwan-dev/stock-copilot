import { describe, expect, it } from "vitest";

it("passes", () => {
  expect(1).toBe(1);
});

it.skip("skipped", () => {
  expect(1).toBe(2);
});
