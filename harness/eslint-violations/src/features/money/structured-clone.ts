import type { Money } from "../../shared/lib/money";
export const copy = (a: Money): Money => structuredClone(a);
