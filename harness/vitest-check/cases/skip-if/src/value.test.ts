import { describe, expect, it } from "vitest";

it("passes", () => {
  expect(1).toBe(1);
});

it.skipIf(true)("skipped", () => {
  expect(1).toBe(2);
});
