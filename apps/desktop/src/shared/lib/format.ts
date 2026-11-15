import type { FormattedNumber, UiText } from "./brand";
import type { Money, Quantity, Ratio, Probability } from "./money";
import { readDecimalForNumberDisplay } from "./money";
import { text } from "./messages";

export type Direction = "up" | "down" | "flat";
type Notation = "full" | "compact";
type NumericInput =
  | {
      readonly kind: "money";
      readonly value: Money;
      readonly notation?: Notation;
    }
  | { readonly kind: "quantity"; readonly value: Quantity }
  | { readonly kind: "percent"; readonly value: Ratio }
  | { readonly kind: "probability"; readonly value: Probability }
  | { readonly kind: "unavailable" };
type Display = {
  readonly kind: "available" | "unavailable";
  readonly text: FormattedNumber | UiText;
};
const group = (digits: string): string =>
  digits.replace(/\B(?=(\d{3})+(?!\d))/gu, ",");
const stripZeroes = (digits: string): string =>
  digits.replace(/^0+(?=\d)/u, "");
const compactInteger = (digits: string): string => {
  if (digits.length <= 4) return group(digits);
  const low = stripZeroes(digits.slice(-4));
  const high = digits.slice(0, -4);
  const unit =
    high.length <= 4 ? text("number.man", "ko") : text("number.eok", "ko");
  const leading = high.length <= 4 ? group(high) : group(high.slice(0, -4));
  const middle = high.length <= 4 ? "" : stripZeroes(high.slice(-4));
  const man =
    middle !== "" && middle !== "0"
      ? ` ${group(middle)}${text("number.man", "ko")}`
      : "";
  const tail = low === "0" ? "" : ` ${group(low)}`;
  return `${leading}${unit}${man}${tail}`;
};
const magnitude = (raw: string, notation: Notation): string => {
  const unsigned = raw.replace(/^-/u, "");
  const [integer = "", fraction] = unsigned.split(".");
  const whole =
    notation === "compact" ? compactInteger(integer) : group(integer);
  if (fraction === undefined) return whole;
  const separator =
    notation === "compact" && integer.length > 4 && integer.endsWith("0000")
      ? " 0"
      : "";
  return `${whole}${separator}.${fraction}`;
};
const sign = (raw: string, direction?: Direction): string => {
  if (/^-?0(?:\.0+)?$/u.test(raw)) return "";
  switch (direction) {
    case "up":
      return "+";
    case "down":
      return "−";
    case "flat":
      return "";
    case undefined:
      return raw.startsWith("-") ? "−" : "";
  }
};
const unitText = (
  input: Exclude<NumericInput, { readonly kind: "unavailable" }>,
): string => {
  switch (input.kind) {
    case "money":
      return input.value.currency === "KRW" ? text("number.won", "ko") : "";
    case "quantity":
      return text("number.shares", "ko");
    case "percent":
    case "probability":
      return "%";
  }
};
export const formatNumber = (
  input: NumericInput,
  direction?: Direction,
): Display => {
  if (input.kind === "unavailable")
    return {
      kind: "unavailable",
      text: text("number.unavailable", "ko"),
    };
  const raw = readDecimalForNumberDisplay(input.value);
  if (!/^-?(0|[1-9]\d*)(\.\d+)?$(?![\s\S])/u.test(raw))
    return {
      kind: "unavailable",
      text: text("number.unavailable", "ko"),
    };
  const notation = input.kind === "money" ? (input.notation ?? "full") : "full";
  const prefix =
    input.kind === "money" && input.value.currency === "USD" ? "$" : "";
  return {
    kind: "available",
    text: `${sign(raw, direction)}${prefix}${magnitude(raw, notation)}${unitText(input)}` as FormattedNumber,
  };
};
