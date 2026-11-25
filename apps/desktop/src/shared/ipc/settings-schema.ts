import { z } from "zod";
import {
  err,
  ok,
  parseChartDecimal,
  parsePercentDisplay,
  type ChartDecimal,
  type PercentDisplay,
  type Result,
} from "../lib";
import { settleWithSchema, type IpcFailure } from "./schema";
import { connectionSchema, type TossConnection } from "./setup-schema";

type ConnectionState = "connected" | "needs-key";
type OptionalKeyState = "configured" | "unconfigured";
export type ToggleState = "off" | "on";
export type OrderPermissionState = "off" | "on";
export type HorizonOption =
  "hours" | "few-days" | "weeks-to-months" | "half-year-or-more";
export type RiskAppetiteOption = "conservative" | "neutral" | "aggressive";
type OrderSwitchOutcomeKind =
  "changed" | "unchanged" | "denied" | "unavailable";

export type SettingsView = {
  readonly tossConnection: ConnectionState;
  readonly horizon: HorizonOption;
  readonly riskAppetite: RiskAppetiteOption;
  readonly instrumentCap: PercentDisplay;
  readonly instrumentWarning: PercentDisplay;
  readonly sectorCap: PercentDisplay;
  readonly excludedSectorCount: ChartDecimal;
  readonly excludedSecurityTypeCount: ChartDecimal;
  readonly orderEnabled: OrderPermissionState;
  readonly aiBudget: PercentDisplay;
  readonly dartKey: OptionalKeyState;
  readonly fredKey: OptionalKeyState;
  readonly allowMarketOrders: ToggleState;
  readonly allowHoursHorizon: ToggleState;
  readonly cardExpiryMinutes: ChartDecimal;
  readonly concentrationThreshold: ChartDecimal;
  readonly autoStartAtLogin: ToggleState;
  readonly accountCount: ChartDecimal;
};

export type OrderSwitchOutcome = {
  readonly kind: OrderSwitchOutcomeKind;
  readonly orderEnabled: OrderPermissionState;
};

const rawSettingsSchema = z
  .object({
    toss_connection: z.enum(["connected", "needs-key"]),
    horizon: z.enum([
      "hours",
      "few-days",
      "weeks-to-months",
      "half-year-or-more",
    ]),
    risk_appetite: z.enum(["conservative", "neutral", "aggressive"]),
    instrument_cap_percent: z.string(),
    instrument_warning_percent: z.string(),
    sector_cap_percent: z.string(),
    excluded_sector_count: z.number().int().nonnegative(),
    excluded_security_type_count: z.number().int().nonnegative(),
    order_enabled: z.enum(["off", "on"]),
    ai_budget_percent: z.string(),
    dart_key: z.enum(["configured", "unconfigured"]),
    fred_key: z.enum(["configured", "unconfigured"]),
    allow_market_orders: z.enum(["off", "on"]),
    allow_hours_horizon: z.enum(["off", "on"]),
    card_expiry_minutes: z.number().int().nonnegative(),
    concentration_warning_threshold: z.number().int().nonnegative(),
    auto_start_at_login: z.enum(["off", "on"]),
    account_count: z.number().int().nonnegative(),
  })
  .strict()
  .readonly();

const rawOrderSwitchSchema = z
  .object({
    kind: z.enum(["changed", "unchanged", "denied", "unavailable"]),
    order_enabled: z.enum(["off", "on"]),
  })
  .strict()
  .readonly();

const commandErrorSchema = z
  .object({ kind: z.enum(["user-fixable", "retryable", "bug", "no-data"]) })
  .strict()
  .readonly();

type CommandOutcome =
  | { readonly status: "ok"; readonly data: unknown }
  | { readonly status: "error"; readonly error: unknown };

type RawSettings = z.infer<typeof rawSettingsSchema>;

const invalidFailure: IpcFailure = { kind: "invalid-response" };
const frameworkFailure: IpcFailure = { kind: "framework" };

const parseErrorOutcome = (error: unknown): Result<never, IpcFailure> => {
  const parsed = commandErrorSchema.safeParse(error);
  return parsed.success
    ? err({ kind: "command", error: parsed.data })
    : err(frameworkFailure);
};

type ParsedPercents = {
  readonly instrumentCap: PercentDisplay;
  readonly instrumentWarning: PercentDisplay;
  readonly sectorCap: PercentDisplay;
  readonly aiBudget: PercentDisplay;
};

type ParsedCounts = {
  readonly excludedSectorCount: ChartDecimal;
  readonly excludedSecurityTypeCount: ChartDecimal;
  readonly cardExpiryMinutes: ChartDecimal;
  readonly concentrationThreshold: ChartDecimal;
  readonly accountCount: ChartDecimal;
};

const parsePercents = (
  raw: RawSettings,
): Result<ParsedPercents, IpcFailure> => {
  const instrumentCap = parsePercentDisplay(raw.instrument_cap_percent);
  const instrumentWarning = parsePercentDisplay(raw.instrument_warning_percent);
  const sectorCap = parsePercentDisplay(raw.sector_cap_percent);
  const aiBudget = parsePercentDisplay(raw.ai_budget_percent);
  if (
    instrumentCap.kind === "err" ||
    instrumentWarning.kind === "err" ||
    sectorCap.kind === "err" ||
    aiBudget.kind === "err"
  )
    return err(invalidFailure);
  return ok({
    instrumentCap: instrumentCap.value,
    instrumentWarning: instrumentWarning.value,
    sectorCap: sectorCap.value,
    aiBudget: aiBudget.value,
  });
};

const parseCounts = (raw: RawSettings): Result<ParsedCounts, IpcFailure> => {
  const excludedSectorCount = parseChartDecimal(
    String(raw.excluded_sector_count),
  );
  const excludedSecurityTypeCount = parseChartDecimal(
    String(raw.excluded_security_type_count),
  );
  const cardExpiryMinutes = parseChartDecimal(String(raw.card_expiry_minutes));
  const concentrationThreshold = parseChartDecimal(
    String(raw.concentration_warning_threshold),
  );
  const accountCount = parseChartDecimal(String(raw.account_count));
  if (
    excludedSectorCount.kind === "err" ||
    excludedSecurityTypeCount.kind === "err" ||
    cardExpiryMinutes.kind === "err" ||
    concentrationThreshold.kind === "err" ||
    accountCount.kind === "err"
  )
    return err(invalidFailure);
  return ok({
    excludedSectorCount: excludedSectorCount.value,
    excludedSecurityTypeCount: excludedSecurityTypeCount.value,
    cardExpiryMinutes: cardExpiryMinutes.value,
    concentrationThreshold: concentrationThreshold.value,
    accountCount: accountCount.value,
  });
};

export const settleSettingsView = (
  outcome: CommandOutcome,
): Result<SettingsView, IpcFailure> => {
  if (outcome.status === "error") return parseErrorOutcome(outcome.error);
  const raw = rawSettingsSchema.safeParse(outcome.data);
  if (!raw.success) return err(invalidFailure);
  const percents = parsePercents(raw.data);
  const counts = parseCounts(raw.data);
  if (percents.kind === "err" || counts.kind === "err")
    return err(invalidFailure);
  return ok({
    tossConnection: raw.data.toss_connection,
    horizon: raw.data.horizon,
    riskAppetite: raw.data.risk_appetite,
    orderEnabled: raw.data.order_enabled,
    dartKey: raw.data.dart_key,
    fredKey: raw.data.fred_key,
    allowMarketOrders: raw.data.allow_market_orders,
    allowHoursHorizon: raw.data.allow_hours_horizon,
    autoStartAtLogin: raw.data.auto_start_at_login,
    ...percents.value,
    ...counts.value,
  });
};

export const settleOrderSwitchOutcome = (
  outcome: CommandOutcome,
): Result<OrderSwitchOutcome, IpcFailure> => {
  if (outcome.status === "error") return parseErrorOutcome(outcome.error);
  const parsed = rawOrderSwitchSchema.safeParse(outcome.data);
  return parsed.success
    ? ok({
        kind: parsed.data.kind,
        orderEnabled: parsed.data.order_enabled,
      })
    : err(invalidFailure);
};

export const settleTossConnection = (
  outcome: CommandOutcome,
): Result<TossConnection, IpcFailure> =>
  settleWithSchema(outcome, connectionSchema);
