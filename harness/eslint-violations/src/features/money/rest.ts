import type { Money } from "../../shared/lib/money";
export const label = (a: Money): "x" => {
  const { currency, ...rest } = a;
  return currency === "KRW" && rest === a ? "x" : "x";
};
