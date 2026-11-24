import { z } from "zod";

export const horizons = ["days", "weeks-months", "half-year-plus"] as const;
export type Horizon = "days" | "weeks-months" | "half-year-plus";
export const riskLevels = ["conservative", "neutral", "aggressive"] as const;
export type RiskLevel = "conservative" | "neutral" | "aggressive";
export const stockCapPercents = ["5", "10", "15", "20", "25", "30"] as const;
export type StockCapPercent = "5" | "10" | "15" | "20" | "25" | "30";
export const sectorCapPercents = ["20", "30", "40", "50", "60"] as const;
export type SectorCapPercent = "20" | "30" | "40" | "50" | "60";
export const excludedKinds = [
  "etf",
  "etn",
  "preferred",
  "spac",
  "reit",
] as const;
export type ExcludedKind = "etf" | "etn" | "preferred" | "spac" | "reit";

export const profileSchema = z
  .object({
    horizon: z.enum(horizons),
    risk: z.enum(riskLevels),
    excluded_kinds: z.array(z.enum(excludedKinds)).readonly(),
    stock_cap_percent: z.enum(stockCapPercents),
    sector_cap_percent: z.enum(sectorCapPercents),
  })
  .strict()
  .readonly();
export type InvestmentProfile = z.infer<typeof profileSchema>;

export type OrderSwitchState = "on" | "off";
export const orderSwitchSchema = z
  .object({ state: z.enum(["on", "off"]) })
  .strict()
  .readonly();
export type OrderSwitch = z.infer<typeof orderSwitchSchema>;
