import type { Money } from "../../shared/lib/money";
export const unit = (a: Money | undefined): "KRW" | "USD" | "none" =>
  a === undefined ? "none" : a.currency;
