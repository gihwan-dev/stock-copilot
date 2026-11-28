import type { EntityId } from "../lib";
import type { ReactElement } from "react";
import { createRoot } from "react-dom/client";

export { detach } from "./detach";
export { useCoreEvent, useNavigationEvent } from "./useCoreEvent";

export const mountApp = (app: Readonly<ReactElement>): void => {
  const container = document.getElementById("root");
  if (container !== null) {
    createRoot(container).render(app);
  }
};
export {
  readCanvasTheme,
  useCanvasChartTheme,
  useColorScheme,
} from "./canvasTheme";
export const focusResearchSection = (
  target: "holdings" | "candidates" | "economy",
  source?: EntityId,
): void => {
  const section = document.getElementById(
    source === undefined ? target : `${target}-${source}`,
  );
  section?.focus({ preventScroll: true });
  section?.scrollIntoView({ block: "start" });
};
