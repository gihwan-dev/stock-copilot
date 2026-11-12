import type { Plain } from "../../shared/lib/brand";
export const echo = (value: Plain): "x" => (value === "a" ? "x" : "x");
