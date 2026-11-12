import type { Money } from "../../shared/lib/money";
export const label = (a: Money): "x" => (String(a) === "x" ? "x" : "x");
