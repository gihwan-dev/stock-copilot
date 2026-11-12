import { describe, expect, it } from "vitest";

it("passes", () => {
  expect(1).toBe(1);
});

it("skipped", (context) => {
  context.skip();
  expect(1).toBe(2);
});
