import { expect, it } from "vitest";
import { testOnly } from "./value";

it("runs", () => {
  expect(testOnly).toBe("t");
});
