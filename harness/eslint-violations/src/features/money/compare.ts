import type { Money } from "../../shared/lib/money";
export const larger = (a: Money, b: Money): "a" | "b" => (a > b ? "a" : "b");
