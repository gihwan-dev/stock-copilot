import { expect, test } from "vitest";
import { text } from "../lib";
import { defaultOrderSettings, defaultProfile } from "./risk-editor";
import {
  orderRequest,
  profileRequest,
  settleRiskSave,
  settleRiskSettings,
} from "./risk-schema";
import { responses } from "./testing/responses";

test("settleRiskSettings turns the recorded defaults into the editor drafts", () => {
  const result = settleRiskSettings({
    status: "ok",
    data: responses.getRiskSettings,
  });
  expect(result).toStrictEqual({
    kind: "ok",
    value: { profile: defaultProfile, order: defaultOrderSettings },
  });
});

test("settleRiskSettings rejects unknown values and classifies command errors", () => {
  expect(
    settleRiskSettings({
      status: "ok",
      data: {
        ...responses.getRiskSettings,
        order: { ...responses.getRiskSettings.order, card_expiry: "60" },
      },
    }).kind,
  ).toBe("err");
  expect(
    settleRiskSettings({ status: "error", error: { kind: "no-data" } }),
  ).toStrictEqual({
    kind: "err",
    error: { kind: "command", error: { kind: "no-data" } },
  });
});

test("settleRiskSave keeps declined and explains a rejection in one sentence", () => {
  expect(
    settleRiskSave({ status: "ok", data: responses.saveInvestorProfile }),
  ).toStrictEqual({ kind: "ok", value: { kind: "declined" } });
  const rejected = settleRiskSave({
    status: "ok",
    data: { kind: "rejected", rejection: "unsupported-exclusion" },
  });
  expect(rejected.kind).toBe("ok");
  if (rejected.kind === "ok" && rejected.value.kind === "rejected") {
    expect(rejected.value.reason).toBe(
      text("risk.rejectUnsupportedExclusion", "ko"),
    );
  }
});

test("draft requests use the snake case wire names", () => {
  expect(profileRequest(defaultProfile)).toStrictEqual({
    horizon: "weeks-to-months",
    risk_appetite: "neutral",
    instrument_cap: "20",
    sector_cap: "40",
    excluded_types: [],
  });
  expect(orderRequest(defaultOrderSettings)).toStrictEqual({
    market_orders: "off",
    hours_horizon: "off",
    card_expiry: "10",
    order_count_limit: "off",
  });
});
