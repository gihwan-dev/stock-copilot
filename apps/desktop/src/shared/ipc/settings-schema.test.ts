import { expect, test } from "vitest";
import {
  settleOrderSwitchOutcome,
  settleSettingsView,
} from "./settings-schema";
import { responses } from "./testing/responses";

test("settleSettingsView parses the recorded default settings response into branded fields", () => {
  const result = settleSettingsView({
    status: "ok",
    data: responses.getSettings,
  });
  expect(result.kind).toBe("ok");
  if (result.kind === "ok") {
    expect(result.value.tossConnection).toBe("needs-key");
    expect(result.value.horizon).toBe("weeks-to-months");
    expect(result.value.riskAppetite).toBe("neutral");
    expect(result.value.orderEnabled).toBe("off");
    expect(result.value.dartKey).toBe("unconfigured");
    expect(result.value.fredKey).toBe("unconfigured");
    expect(result.value.autoStartAtLogin).toBe("on");
  }
});

test("settleSettingsView rejects malformed percent strings and classifies command errors", () => {
  expect(
    settleSettingsView({
      status: "ok",
      data: { ...responses.getSettings, instrument_cap_percent: "bad" },
    }),
  ).toStrictEqual({ kind: "err", error: { kind: "invalid-response" } });
  expect(
    settleSettingsView({
      status: "error",
      error: responses.ipcErrors[1],
    }),
  ).toStrictEqual({
    kind: "err",
    error: { kind: "command", error: { kind: "retryable" } },
  });
});

test("settleOrderSwitchOutcome parses recorded denied outcome and rejects unknown fields", () => {
  expect(
    settleOrderSwitchOutcome({
      status: "ok",
      data: responses.setOrderPermission,
    }),
  ).toStrictEqual({
    kind: "ok",
    value: { kind: "denied", orderEnabled: "off" },
  });
  expect(
    settleOrderSwitchOutcome({
      status: "ok",
      data: { kind: "denied", order_enabled: "off", extra: "probe" },
    }),
  ).toStrictEqual({ kind: "err", error: { kind: "invalid-response" } });
});
