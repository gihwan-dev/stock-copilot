import { canvasProbe } from "../platform/canvasTheme";

export const probe = (): "ok" | "other" =>
  Number("1") === 1 ? canvasProbe() : "other";
