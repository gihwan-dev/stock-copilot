import type { Money } from "./money";
export const amount = (a: Money): "x" => (Number(a) === 0 ? "x" : "x");
