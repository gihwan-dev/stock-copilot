import {
  parseInputValue,
  err,
  type InputValue,
  type BrandError,
  type Result,
} from "../lib";
import type { ChangeEvent, KeyboardEvent } from "react";

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
