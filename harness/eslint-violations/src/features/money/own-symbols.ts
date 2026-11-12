import type { Money } from "../../shared/lib/money";
export const keys = (a: Money): "x" =>
  Object.getOwnPropertySymbols(a).length === 0 ? "x" : "x";
