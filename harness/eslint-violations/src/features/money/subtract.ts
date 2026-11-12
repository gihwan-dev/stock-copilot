import type { Money } from "../../shared/lib/money";
export const gap = (a: Money, b: Money): "x" => (a - b === 0 ? "x" : "x");
