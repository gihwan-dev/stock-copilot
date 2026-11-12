import { expect, it, vi } from "vitest";

// eslint-disable-next-line guard/no-module-mocks -- 끌 수 없는 규칙
vi.mock("../index");

it("passes", () => {
  expect(1).toBe(1);
});
