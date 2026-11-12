import type { Money } from "../../../../apps/desktop/src/shared/lib/money";
export const total = (a: Money, b: Money): "x" =>
  parseFloat(a) + Number(b) === 0 ? "x" : "x";
