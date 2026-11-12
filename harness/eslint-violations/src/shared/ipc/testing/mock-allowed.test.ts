import { expect, it, vi } from "vitest";

vi.mock("../index");

it("passes", () => {
  expect(1).toBe(1);
});
