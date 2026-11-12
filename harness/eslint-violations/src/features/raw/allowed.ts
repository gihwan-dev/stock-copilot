import type { Ticker } from "../../shared/lib/brand";
export type Side = "buy" | "sell";
export const pick = (side: Side, items: readonly Ticker[]): "first" | "none" =>
  items.filter((item) => item.length > 0 && side === "buy").length > 0
    ? "first"
    : "none";
