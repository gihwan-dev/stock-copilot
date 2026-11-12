import type { Money } from "../../shared/lib/money";
export const total = (a: Money, b: Money): "x" => (a + b === 0 ? "x" : "x");
