import { expect, it, vi } from "vitest";

it("stubs a global", () => {
  vi.stubGlobal("leakedMarker", "set");
  expect("leakedMarker" in globalThis).toBe(true);
});

it("does not see the stub of the previous test", () => {
  expect("leakedMarker" in globalThis).toBe(false);
});
