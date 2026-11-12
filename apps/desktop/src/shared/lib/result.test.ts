import { expect, it } from "vitest";
import { err, ok } from "./result";

it("ok carries the value", () => {
  expect(ok("a")).toStrictEqual({ kind: "ok", value: "a" });
});

it("err carries the error", () => {
  expect(err("e")).toStrictEqual({ kind: "err", error: "e" });
});
