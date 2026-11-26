import {
  clampPanelShare,
  parseInputValue,
  parseFreeText,
  err,
  type InputValue,
  type FreeText,
  type BrandError,
  type PanelBounds,
  type PanelShare,
  type Result,
} from "../lib";
import {
  useEffect,
  useState,
  useSyncExternalStore,
  type ChangeEvent,
  type KeyboardEvent,
  type RefObject,
} from "react";

export const inputEventValue = (
  event: Readonly<ChangeEvent<HTMLInputElement | HTMLSelectElement>>,
): Result<InputValue, BrandError> => parseInputValue(event.currentTarget.value);
export const inputCommit = (
  event: Readonly<KeyboardEvent<HTMLInputElement>>,
  composition: "idle" | "composing" | "ending" = "idle",
): "commit" | "ignore" => {
  if (event.key !== "Enter") return "ignore";
  if (
    event.nativeEvent.isComposing ||
    legacyComposition(event.nativeEvent) === "composing" ||
    composition !== "idle"
  ) {
    event.preventDefault();
    return "ignore";
  }
  return "commit";
};
const legacyComposition = (event: {
  readonly keyCode: KeyboardEvent["keyCode"];
}): "composing" | "idle" => (event.keyCode === 229 ? "composing" : "idle");
export const pasteInputValue = (
  receive: (result: Result<InputValue, BrandError | ClipboardError>) => void,
): void => {
  const source = clipboard();
  if (source === undefined) {
    receive(err({ kind: "clipboard-unavailable" }));
    return;
  }
  source.readText().then(
    (raw): void => {
      receive(parseInputValue(raw.trim()));
    },
    (): void => {
      receive(err({ kind: "clipboard-denied" }));
    },
  );
};
type ClipboardError = {
  readonly kind: "clipboard-unavailable" | "clipboard-denied";
};

const clipboard = (): Clipboard | undefined => navigator.clipboard;

export const focusDialogInput = (
  ref: Readonly<RefObject<HTMLDivElement | null>>,
): void => {
  const popup = ref.current;
  const target =
    popup?.querySelector<HTMLElement>(
      'input:not([type="hidden"]):not([disabled]),select:not([disabled]),textarea:not([disabled])',
    ) ??
    popup?.querySelector<HTMLElement>("button:not([disabled])") ??
    popup;
  target?.focus();
};

export const chartNavigation = (
  event: Readonly<KeyboardEvent<HTMLButtonElement>>,
): "next" | "previous" | "first" | "last" | "ignore" => {
  if (event.key === "ArrowRight" || event.key === "ArrowDown") {
    event.preventDefault();
    return "next";
  }
  if (event.key === "ArrowLeft" || event.key === "ArrowUp") {
    event.preventDefault();
    return "previous";
  }
  if (event.key === "Home") {
    event.preventDefault();
    return "first";
  }
  if (event.key === "End") {
    event.preventDefault();
    return "last";
  }
  return "ignore";
};

export const chatEventValue = (
  event: Readonly<ChangeEvent<HTMLTextAreaElement>>,
): Result<FreeText, BrandError> => parseFreeText(event.currentTarget.value);
export const chatCommit = (
  event: Readonly<KeyboardEvent<HTMLTextAreaElement>>,
  composition: "idle" | "composing" | "ending",
): "commit" | "ignore" => {
  if (event.key !== "Enter" || event.shiftKey) return "ignore";
  if (
    event.nativeEvent.isComposing ||
    legacyComposition(event.nativeEvent) === "composing" ||
    composition !== "idle"
  ) {
    if (composition === "ending") event.preventDefault();
    return "ignore";
  }
  event.preventDefault();
  return "commit";
};

export const shellDismiss = (
  event: Readonly<KeyboardEvent<HTMLDivElement>>,
): "dismiss" | "ignore" => {
  if (event.key !== "Escape") return "ignore";
  event.preventDefault();
  return "dismiss";
};

const chatWidthKey = "stock-copilot.chat-share";
const wideQuery = "(min-width: 80rem)";
const chatPixels = 384;
const sidebarPixels = 232;
const fallbackShare = 33;

const viewportShare = (): number => {
  const width =
    typeof document === "undefined"
      ? 0
      : document.documentElement.clientWidth - sidebarPixels;
  return width > 0 ? (chatPixels / width) * 100 : fallbackShare;
};
const chatBounds: PanelBounds = { min: 25, max: 50, initial: fallbackShare };

export const chatPanelLimits = {
  min: clampPanelShare(25, chatBounds),
  max: clampPanelShare(50, chatBounds),
} as const;

const subscribeWide = (onChange: () => void): (() => void) => {
  const media =
    typeof matchMedia === "function" ? matchMedia(wideQuery) : undefined;
  media?.addEventListener("change", onChange);
  return (): void => {
    media?.removeEventListener("change", onChange);
  };
};
const readWide = (): "resizable" | "static" =>
  typeof ResizeObserver === "function" &&
  typeof matchMedia === "function" &&
  matchMedia(wideQuery).matches
    ? "resizable"
    : "static";
export const useShellResize = (): "resizable" | "static" =>
  useSyncExternalStore(subscribeWide, readWide, (): "static" => "static");

export const initialChatShare = (): PanelShare =>
  clampPanelShare(viewportShare(), chatBounds);
export const readChatShare = (): PanelShare => {
  const stored =
    typeof localStorage === "undefined"
      ? null
      : localStorage.getItem(chatWidthKey);
  return stored === null
    ? initialChatShare()
    : clampPanelShare(stored, chatBounds);
};
const writeStorage = (write: (storage: Storage) => void): void => {
  if (typeof localStorage === "undefined") return;
  new Promise<void>((resolve) => {
    write(localStorage);
    resolve();
  }).catch((): void => undefined);
};
export const saveChatShare = (size: {
  readonly asPercentage: number;
}): void => {
  if (size.asPercentage <= 0) return;
  writeStorage((storage): void => {
    storage.setItem(
      chatWidthKey,
      clampPanelShare(size.asPercentage, chatBounds),
    );
  });
};
export const clearChatShare = (): void => {
  writeStorage((storage): void => {
    storage.removeItem(chatWidthKey);
  });
};

const followThreshold = 48;
const followState = (viewport: Readonly<HTMLElement>): "pinned" | "free" =>
  viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight <=
  followThreshold
    ? "pinned"
    : "free";
const scrollViewport = (id: string): HTMLElement | null =>
  typeof document === "undefined"
    ? null
    : (document
        .getElementById(id)
        ?.querySelector<HTMLElement>("[data-scroll-viewport]") ?? null);
export const useFollowEnd = (id: string, mode: "end" | "none"): void => {
  const [follow, setFollow] = useState<"pinned" | "free">("pinned");
  useEffect(() => {
    const viewport = scrollViewport(id);
    if (mode !== "end" || viewport === null) return undefined;
    const onScroll = (): void => {
      setFollow(followState(viewport));
    };
    viewport.addEventListener("scroll", onScroll, { passive: true });
    return (): void => {
      viewport.removeEventListener("scroll", onScroll);
    };
  }, [id, mode]);
  useEffect(() => {
    const viewport = scrollViewport(id);
    if (mode !== "end" || viewport === null || follow === "free")
      return undefined;
    const toEnd = (): void => {
      if ("scrollTo" in viewport)
        viewport.scrollTo({ top: viewport.scrollHeight });
    };
    const observer =
      typeof MutationObserver === "function"
        ? new MutationObserver(toEnd)
        : undefined;
    observer?.observe(viewport, {
      childList: true,
      subtree: true,
      characterData: true,
    });
    toEnd();
    return (): void => {
      observer?.disconnect();
    };
  }, [id, mode, follow]);
};
