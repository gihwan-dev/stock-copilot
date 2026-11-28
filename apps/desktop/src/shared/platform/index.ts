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
  target:
    | "holdings"
    | "candidates"
    | "economy"
    | "positions"
    | "daily-briefing"
    | "signals"
    | "allocation",
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

const layoutQueries = [
  ["three-columns", "(min-width: 140rem)"],
  ["two-columns", "(min-width: 100rem)"],
  ["summary-row", "(min-width: 80rem)"],
] as const;
type WindowLayout = "three-columns" | "two-columns" | "summary-row" | "compact";

const subscribeLayout = (change: () => void): (() => void) => {
  const queries =
    typeof matchMedia === "function"
      ? layoutQueries.map(([, query]) => matchMedia(query))
      : [];
  queries.forEach((query) => {
    query.addEventListener("change", change);
  });
  return (): void => {
    queries.forEach((query) => {
      query.removeEventListener("change", change);
    });
  };
};

const readLayout = (): WindowLayout =>
  typeof matchMedia === "function"
    ? (layoutQueries.find(([, query]) => matchMedia(query).matches)?.[0] ??
      "compact")
    : "compact";

export const useWindowLayout = (): WindowLayout =>
  useSyncExternalStore(
    subscribeLayout,
    readLayout,
    (): WindowLayout => "compact",
  );
