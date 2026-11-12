import type { Money } from "../../shared/lib/money";
export const same = (a: Money, b: Money): "y" | "n" => (a === b ? "y" : "n");
