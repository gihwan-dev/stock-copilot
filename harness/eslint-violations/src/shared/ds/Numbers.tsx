import { plain } from "../lib/format";
export const displayed = (): "ok" | "fallback" =>
  plain("1") === "1" ? "ok" : "fallback";
