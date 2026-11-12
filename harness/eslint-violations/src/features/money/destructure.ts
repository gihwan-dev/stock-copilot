import { moneyKey, type Money } from "../../shared/lib/money";
export const label = (a: Money): "x" => {
  const { [moneyKey]: amount } = a;
  return amount === "1" ? "x" : "x";
};
