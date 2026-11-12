import type { Money } from "../../shared/lib/money";
export const larger = <T extends Money>(a: T, b: T): "a" | "b" =>
  a > b ? "a" : "b";
