import { plainLabel, plainValue } from "../../shared/lib/brand";
export const run = (): "x" | "y" =>
  plainLabel(plainValue) === "a" ? "x" : "y";
