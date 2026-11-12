import { describe, expect, it } from "vitest";

it("passes", () => {
  expect(1).toBe(1);
});

describe.skip("group", () => {
  it("inner", () => {
    expect(1).toBe(2);
  });
});
