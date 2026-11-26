import { expect, test } from "vitest";
import { settleAutoStart } from "./autostart-call";
import { wire } from "./schema";
import { responses } from "./testing/responses";

test("settleAutoStart returns the applied login start state", () => {
  expect(
    settleAutoStart({ status: "ok", data: wire(responses.setAutoStart) }),
  ).toStrictEqual({ kind: "ok", value: "off" });
  expect(
    settleAutoStart({
      status: "ok",
      data: wire({ auto_start_at_login: "maybe" }),
    }).kind,
  ).toBe("err");
});
