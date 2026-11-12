import type { Money } from "../../shared/lib/money";
export const label = (a: Money): "x" => (JSON.stringify(a) === "x" ? "x" : "x");
