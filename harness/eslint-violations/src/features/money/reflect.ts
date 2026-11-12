import type { Money } from "../../shared/lib/money";
export const keys = (a: Money): "x" =>
  Reflect.ownKeys(a).length === 0 ? "x" : "x";
