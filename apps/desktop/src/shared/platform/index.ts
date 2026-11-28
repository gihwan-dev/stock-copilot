import type { EntityId } from "../lib";
import { useSyncExternalStore, type ReactElement } from "react";
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
  reveal?: () => void,
): void => {
  const focus = (): void => {
    const section = document.getElementById(
      source === undefined ? target : `${target}-${source}`,
    );
    section?.focus({ preventScroll: true });
    section?.scrollIntoView({ block: "start" });
  };
  if (reveal === undefined) {
    focus();
  } else {
    reveal();
    requestAnimationFrame(focus);
  }
};

const wideQuery = "(min-width: 80rem)";

const subscribeWide = (onChange: () => void): (() => void) => {
  const media =
    typeof matchMedia === "function" ? matchMedia(wideQuery) : undefined;
  media?.addEventListener("change", onChange);
  return (): void => {
    media?.removeEventListener("change", onChange);
  };
};
const windowLayout = (): "wide" | "compact" =>
  typeof matchMedia === "function" && matchMedia(wideQuery).matches
    ? "wide"
    : "compact";

export const useWideWindow = (): "wide" | "compact" =>
  useSyncExternalStore(subscribeWide, windowLayout, (): "compact" => "compact");
