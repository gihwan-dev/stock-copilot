import type { Money } from "../../shared/lib/money";
export const label = (a: Money): "x" => (`${a}` === "x" ? "x" : "x");
