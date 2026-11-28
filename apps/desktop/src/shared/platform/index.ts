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
const scrollViewport = (target: Readonly<Element>): Element | undefined =>
  Array.from(document.querySelectorAll("[data-scroll-viewport]")).findLast(
    (node) => node.contains(target) && node.scrollHeight > node.clientHeight,
  );

const revealNearest = (target: Readonly<HTMLElement>): void => {
  target.focus({ preventScroll: true });
  const scroller = scrollViewport(target);
  if (scroller === undefined) {
    target.scrollIntoView({ block: "nearest" });
    return;
  }
  const viewport = scroller.getBoundingClientRect();
  const item = target.getBoundingClientRect();
  const margin = 16;
  const top = Math.max(0, viewport.top) + margin;
  const bottom =
    Math.min(document.documentElement.clientHeight, viewport.bottom) - margin;
  if (item.top < top) scroller.scrollBy({ top: item.top - top });
  else if (item.bottom > bottom)
    scroller.scrollBy({ top: item.bottom - bottom });
};

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
    if (section === null) return;
    if (source === undefined) {
      section.focus({ preventScroll: true });
      section.scrollIntoView({ block: "start" });
    } else revealNearest(section);
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

const focusNewExcerpt = (item: Readonly<HTMLLIElement> | undefined): void => {
  if (item === undefined) return;
  const citation = item.querySelector<HTMLElement>('[role="link"], a[href]');
  revealNearest(citation ?? item);
};

export const preserveExcerptPosition = (
  next: "open" | "closed",
  activation: {
    readonly trigger: EntityId;
    readonly input: "keyboard" | "pointer";
  },
  change: (value: "open" | "closed") => void,
): void => {
  const trigger = document.getElementById(activation.trigger);
  const controlled = trigger?.getAttribute("aria-controls");
  const content = document.getElementById(controlled ?? "");
  if (trigger === null || content === null) {
    change(next);
    return;
  }
  const scroller = scrollViewport(trigger);
  const initialScroll = scroller?.scrollTop ?? 0;
  const initialTop = trigger.getBoundingClientRect().top;
  const shown = content.querySelectorAll("li").length;
  change(next);
  requestAnimationFrame(() => {
    if (next === "open" && activation.input === "keyboard") {
      focusNewExcerpt(Array.from(content.querySelectorAll("li")).at(shown));
    } else if (scroller !== undefined) {
      const relativeTop = trigger.getBoundingClientRect().top - initialTop;
      scroller.scrollTo({
        top: next === "open" ? initialScroll : scroller.scrollTop + relativeTop,
      });
    }
  });
};
