import { expect, it, vi } from "vitest";

vi.mock("./value");

it("passes", () => {
  expect(1).toBe(1);
});
