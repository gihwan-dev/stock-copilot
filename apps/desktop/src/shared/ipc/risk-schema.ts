import { z } from "zod";
import {
  emptyFreeText,
  err,
  ok,
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
import type { IpcFailure } from "./schema";

type CommandOutcome =
  | { readonly status: "ok"; readonly data: unknown }
  | { readonly status: "error"; readonly error: unknown };

const invalidFailure: IpcFailure = { kind: "invalid-response" };

const commandErrorSchema = z
  .object({ kind: z.enum(["user-fixable", "retryable", "bug", "no-data"]) })
  .strict()
  .readonly();

const toggle = z.enum(["off", "on"]);

const profileSchema = z
  .object({
    horizon: z.enum([
      "hours",
      "few-days",
      "weeks-to-months",
      "half-year-or-more",
    ]),
    risk_appetite: z.enum(["conservative", "neutral", "aggressive"]),
    instrument_cap: z.enum(["10", "15", "20", "25", "30"]),
    sector_cap: z.enum(["30", "40", "50"]),
    excluded_types: z.array(
      z.enum(["etf", "etn", "preferred", "spac", "reit"]),
    ),
  })
  .strict()
  .readonly();

const orderSchema = z
  .object({
    market_orders: toggle,
    hours_horizon: toggle,
    card_expiry: z.enum(["5", "10", "15", "30"]),
    order_count_limit: z.enum(["off", "3", "5", "10"]),
  })
  .strict()
  .readonly();

const viewSchema = z
  .object({ profile: profileSchema, order: orderSchema })
  .strict()
  .readonly();

const saveSchema = z
  .object({
    kind: z.enum(["saved", "unchanged", "declined", "unavailable", "rejected"]),
    rejection: z.enum([
      "none",
      "invalid-profile",
      "unsupported-exclusion",
      "unsupported-order-setting",
    ]),
  })
  .strict()
  .readonly();

export type RiskSettingsView = {
  readonly profile: ProfileDraft;
  readonly order: OrderSettingsDraft;
};

const failureOf = (error: unknown): Result<never, IpcFailure> => {
  const parsed = commandErrorSchema.safeParse(error);
  return parsed.success
    ? err({ kind: "command", error: parsed.data })
    : err(invalidFailure);
};

export const settleRiskSettings = (
  outcome: CommandOutcome,
): Result<RiskSettingsView, IpcFailure> => {
  if (outcome.status === "error") return failureOf(outcome.error);
  const parsed = viewSchema.safeParse(outcome.data);
  if (!parsed.success) return err(invalidFailure);
  const { profile, order } = parsed.data;
  return ok({
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
  });
};

const reasonText = (
  rejection: z.infer<typeof saveSchema>["rejection"],
): UiText => {
  switch (rejection) {
    case "unsupported-exclusion":
      return text("risk.rejectUnsupportedExclusion", "ko");
    case "unsupported-order-setting":
      return text("risk.rejectUnsupportedOrderSetting", "ko");
    case "invalid-profile":
    case "none":
      return text("risk.rejectInvalidProfile", "ko");
  }
};

const asFreeText = (value: UiText): FreeText => {
  const parsed = parseFreeText(value);
  return parsed.kind === "ok" ? parsed.value : emptyFreeText;
};

export const settleRiskSave = (
  outcome: CommandOutcome,
): Result<RiskSaveOutcome, IpcFailure> => {
  if (outcome.status === "error") return failureOf(outcome.error);
  const parsed = saveSchema.safeParse(outcome.data);
  if (!parsed.success) return err(invalidFailure);
  return parsed.data.kind === "rejected"
    ? ok({
        kind: "rejected",
        reason: asFreeText(reasonText(parsed.data.rejection)),
      })
    : ok({ kind: parsed.data.kind });
};

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
