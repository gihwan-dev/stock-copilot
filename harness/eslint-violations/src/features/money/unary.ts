import type { Money } from "../../shared/lib/money";
export const negate = (a: Money): "x" => (-a === 0 ? "x" : "x");
