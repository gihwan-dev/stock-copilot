import type { Money } from "../../shared/lib/money";
export const label = (a: Money): "x" => (a + "KRW" === "x" ? "x" : "x");
