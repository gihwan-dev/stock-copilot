import {
  parseInputValue,
  err,
  type InputValue,
  type BrandError,
  type Result,
} from "../lib";
import type { ChangeEvent, KeyboardEvent, RefObject } from "react";

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
