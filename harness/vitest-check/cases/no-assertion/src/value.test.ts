import { expect, it } from "vitest";

it("passes", () => {
  expect(1).toBe(1);
});

it("checks nothing", () => {
  const value = 1;
  return value === 1 ? undefined : undefined;
});
