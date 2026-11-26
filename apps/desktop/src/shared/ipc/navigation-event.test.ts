import { expect, test } from "vitest";
import { parseUiEvent } from "./event-schema";

test("the menu bar data status request parses as a navigation event", () => {
  expect(parseUiEvent("ShowDataStatus")).toStrictEqual({
    kind: "ok",
    value: { category: "navigation", kind: "ShowDataStatus" },
  });
  expect(parseUiEvent({ ShowDataStatus: "x" }).kind).toBe("err");
});
