import type { Money } from "./money";
export const total = (a: Money, b: Money): "x" =>
  parseFloat(a) + Number(b) === 0 ? "x" : "x";
