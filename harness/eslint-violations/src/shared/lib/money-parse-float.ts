import type { Money } from "./money";
export const amount = (a: Money): "x" => (parseFloat(a) === 0 ? "x" : "x");
