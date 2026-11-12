import { expect, it, vi } from "vitest";

it("spies", () => {
  const target = { run: (): "a" => "a" };
  const spy = vi.spyOn(target, "run");
  expect(spy).toBeDefined();
});
