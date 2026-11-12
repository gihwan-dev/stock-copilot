import { expect, it, vi } from "vitest";

vi.doMock("./value");

it("passes", () => {
  expect(1).toBe(1);
});
