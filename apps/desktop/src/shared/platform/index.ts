import {
  parseInputValue,
  err,
  type InputValue,
  type BrandError,
  type Result,
} from "../lib";
import type { ChangeEvent, KeyboardEvent, ReactElement } from "react";
import { createRoot } from "react-dom/client";

export const mountApp = (app: Readonly<ReactElement>): void => {
  const container = document.getElementById("root");
  if (container !== null) {
    createRoot(container).render(app);
  }
};

export const inputEventValue = (
  event: Readonly<ChangeEvent<HTMLInputElement | HTMLSelectElement>>,
): Result<InputValue, BrandError> => parseInputValue(event.currentTarget.value);
export const inputCommit = (
  event: Readonly<KeyboardEvent<HTMLInputElement>>,
): "commit" | "ignore" =>
  event.key === "Enter" && !event.nativeEvent.isComposing ? "commit" : "ignore";
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
