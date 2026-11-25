import { z } from "zod";
import {
  emptyFreeText,
  parseFreeText,
  text,
  type FreeText,
  type Result,
  type UiText,
} from "../lib";
import type {
  OrderSettingsDraft,
  ProfileDraft,
  RiskSaveOutcome,
} from "./risk-editor";
import type { UiOrderSettingsDraft, UiProfileDraft } from "./bindings";
import { settleWithSchema, type IpcFailure, type WireOutcome } from "./schema";

const toggle = z.enum(["off", "on"]);

const profileSchema = z.strictObject({
  horizon: z.enum([
    "hours",
    "few-days",
    "weeks-to-months",
    "half-year-or-more",
  ]),
  risk_appetite: z.enum(["conservative", "neutral", "aggressive"]),
  instrument_cap: z.enum(["10", "15", "20", "25", "30"]),
  sector_cap: z.enum(["30", "40", "50"]),
  excluded_types: z.array(z.enum(["etf", "etn", "preferred", "spac", "reit"])),
});

const orderSchema = z.strictObject({
  market_orders: toggle,
  hours_horizon: toggle,
  card_expiry: z.enum(["5", "10", "15", "30"]),
  order_count_limit: z.enum(["off", "3", "5", "10"]),
});

export type RiskSettingsView = {
  readonly profile: ProfileDraft;
  readonly order: OrderSettingsDraft;
};

const viewSchema = z
  .strictObject({ profile: profileSchema, order: orderSchema })
  .transform(({ profile, order }): RiskSettingsView => ({
    profile: {
      horizon: profile.horizon,
      riskAppetite: profile.risk_appetite,
      instrumentCap: profile.instrument_cap,
      sectorCap: profile.sector_cap,
      excludedTypes: [...profile.excluded_types],
    },
    order: {
      marketOrders: order.market_orders,
      hoursHorizon: order.hours_horizon,
      cardExpiry: order.card_expiry,
      orderCountLimit: order.order_count_limit,
    },
  }));

const asFreeText = (value: UiText): FreeText => {
  const parsed = parseFreeText(value);
  return parsed.kind === "ok" ? parsed.value : emptyFreeText;
};

const rejectionText = {
  none: "risk.rejectInvalidProfile",
  "invalid-profile": "risk.rejectInvalidProfile",
  "unsupported-exclusion": "risk.rejectUnsupportedExclusion",
  "unsupported-order-setting": "risk.rejectUnsupportedOrderSetting",
} as const;

const saveSchema = z
  .strictObject({
    kind: z.enum(["saved", "unchanged", "declined", "unavailable", "rejected"]),
    rejection: z.enum([
      "none",
      "invalid-profile",
      "unsupported-exclusion",
      "unsupported-order-setting",
    ]),
  })
  .transform((raw): RiskSaveOutcome =>
    raw.kind === "rejected"
      ? {
          kind: "rejected",
          reason: asFreeText(text(rejectionText[raw.rejection], "ko")),
        }
      : { kind: raw.kind },
  );

export const settleRiskSettings = (
  outcome: WireOutcome,
): Result<RiskSettingsView, IpcFailure> =>
  settleWithSchema(outcome, viewSchema);

export const settleRiskSave = (
  outcome: WireOutcome,
): Result<RiskSaveOutcome, IpcFailure> => settleWithSchema(outcome, saveSchema);

export const profileRequest = (draft: ProfileDraft): UiProfileDraft => ({
  horizon: draft.horizon,
  risk_appetite: draft.riskAppetite,
  instrument_cap: draft.instrumentCap,
  sector_cap: draft.sectorCap,
  excluded_types: [...draft.excludedTypes],
});

export const orderRequest = (
  draft: OrderSettingsDraft,
): UiOrderSettingsDraft => ({
  market_orders: draft.marketOrders,
  hours_horizon: draft.hoursHorizon,
  card_expiry: draft.cardExpiry,
  order_count_limit: draft.orderCountLimit,
});
