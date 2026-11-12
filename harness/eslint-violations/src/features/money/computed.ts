import { moneyKey, type Money } from "../../shared/lib/money";
export const label = (a: Money): "x" => (a[moneyKey] === "1" ? "x" : "x");
