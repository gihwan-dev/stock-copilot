import type { FreeText, Result } from "../lib";
import type { IpcFailure } from "./schema";
import type {
  HorizonOption,
  RiskAppetiteOption,
  ToggleState,
} from "./settings-schema";

export const profileHorizons = [
  "few-days",
  "weeks-to-months",
  "half-year-or-more",
  "hours",
] as const;
export const riskAppetites = ["conservative", "neutral", "aggressive"] as const;
export const instrumentCaps = ["10", "15", "20", "25", "30"] as const;
export type InstrumentCap = "10" | "15" | "20" | "25" | "30";
export const sectorCaps = ["30", "40", "50"] as const;
export type SectorCap = "30" | "40" | "50";
export const excludableTypes = [
  "etf",
  "etn",
  "preferred",
  "spac",
  "reit",
] as const;
export type ExcludableType = "etf" | "etn" | "preferred" | "spac" | "reit";
export const cardExpiries = ["5", "10", "15", "30"] as const;
export type CardExpiry = "5" | "10" | "15" | "30";
export const orderCountLimits = ["off", "3", "5", "10"] as const;
export type OrderCountLimit = "off" | "3" | "5" | "10";

export type ProfileDraft = {
  readonly horizon: HorizonOption;
  readonly riskAppetite: RiskAppetiteOption;
  readonly instrumentCap: InstrumentCap;
  readonly sectorCap: SectorCap;
  readonly excludedTypes: readonly ExcludableType[];
};

export type OrderSettingsDraft = {
  readonly marketOrders: ToggleState;
  readonly hoursHorizon: ToggleState;
  readonly cardExpiry: CardExpiry;
  readonly orderCountLimit: OrderCountLimit;
};

export const defaultOrderSettings: OrderSettingsDraft = {
  marketOrders: "off",
  hoursHorizon: "off",
  cardExpiry: "10",
  orderCountLimit: "off",
};

export const defaultProfile: ProfileDraft = {
  horizon: "weeks-to-months",
  riskAppetite: "neutral",
  instrumentCap: "20",
  sectorCap: "40",
  excludedTypes: [],
};

export type RiskSaveOutcome =
  | { readonly kind: "saved" }
  | { readonly kind: "unchanged" }
  | { readonly kind: "declined" }
  | { readonly kind: "unavailable" }
  | { readonly kind: "rejected"; readonly reason: FreeText };

type Load<T> = () => Promise<Result<T, IpcFailure>>;

export type RiskEditorSources = {
  readonly investorProfile: Load<ProfileDraft>;
  readonly orderSettings: Load<OrderSettingsDraft>;
  readonly saveInvestorProfile: (
    draft: ProfileDraft,
  ) => Promise<Result<RiskSaveOutcome, IpcFailure>>;
  readonly saveOrderSettings: (
    draft: OrderSettingsDraft,
  ) => Promise<Result<RiskSaveOutcome, IpcFailure>>;
};
