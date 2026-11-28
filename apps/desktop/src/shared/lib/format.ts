import type {
  ChartDecimal,
  Count,
  EntityId,
  FormattedNumber,
  FreeText,
  Ticker,
  Timestamp,
  UiText,
} from "./brand";
import type { Money, Price, Quantity, PercentDisplay } from "./money";
import { readDecimalForNumberDisplay } from "./money";
import { text, type MessageKey } from "./messages";

type Direction = "up" | "down" | "flat";
type Notation = "full" | "compact";
type NumericInput =
  | {
      readonly kind: "money";
      readonly value: Money | Price;
      readonly notation?: Notation;
    }
  | { readonly kind: "quantity"; readonly value: Quantity }
  | { readonly kind: "percent" | "probability"; readonly value: PercentDisplay }
  | { readonly kind: "decimal"; readonly value: ChartDecimal }
  | { readonly kind: "unavailable" };
export type Display =
  | { readonly kind: "available"; readonly text: FormattedNumber }
  | { readonly kind: "unavailable"; readonly text: UiText };
const unavailable = (): Display => ({
  kind: "unavailable",
  text: text("number.unavailable", "ko"),
});
const group = (digits: string): string =>
  digits.replace(/\B(?=(\d{3})+(?!\d))/gu, ",");
const compactPart = (part: {
  readonly digits: string;
  readonly unit: MessageKey;
}): string => {
  if (/^0*$/u.test(part.digits)) return "";
  return `${group(part.digits.replace(/^0+/u, ""))}${text(part.unit, "ko")}`;
};
const compactInteger = (digits: string): string => {
  if (digits.length <= 4) return group(digits);
  return (
    [
      { digits: digits.slice(0, -12), unit: "number.jo" },
      { digits: digits.slice(-12, -8), unit: "number.eok" },
      { digits: digits.slice(-8, -4), unit: "number.man" },
      { digits: digits.slice(-4), unit: "number.empty" },
    ] as const
  )
    .map(compactPart)
    .filter((part): boolean => part !== "")
    .join(" ");
};
export const directionStatus = (direction: unknown): "valid" | "invalid" => {
  switch (direction) {
    case "up":
    case "down":
    case "flat":
      return "valid";
  }
  return "invalid";
};
const nonzeroDirection = (
  raw: string,
  direction: Direction,
): "matching" | "mismatched" => {
  switch (direction) {
    case "up":
      return raw.startsWith("-") ? "mismatched" : "matching";
    case "down":
      return raw.startsWith("-") ? "matching" : "mismatched";
    case "flat":
      return "mismatched";
  }
};
const directionMatches = (
  raw: string,
  direction?: Direction,
): "matching" | "mismatched" => {
  if (direction === undefined) return "matching";
  if (directionStatus(direction) === "invalid") return "mismatched";
  if (/^-?0(?:\.0+)?$/u.test(raw)) return "matching";
  return nonzeroDirection(raw, direction);
};
const sign = (raw: string, direction?: Direction): string => {
  if (/^-?0(?:\.0+)?$/u.test(raw)) return "";
  if (raw.startsWith("-")) return "−";
  return direction === "up" ? "+" : "";
};
const magnitude = (
  raw: string,
  input: Exclude<NumericInput, { readonly kind: "unavailable" }>,
): string => {
  const unsigned = raw.replace(/^-/u, "");
  const compact =
    input.kind === "money" &&
    input.value.currency === "KRW" &&
    input.notation === "compact" &&
    !raw.includes(".");
  if (compact) return compactInteger(unsigned);
  const [integer = "", fraction] = unsigned.split(".");
  return fraction === undefined
    ? group(integer)
    : `${group(integer)}.${fraction}`;
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
    case "decimal":
      return "";
  }
};
const currencyStatus = (currency: unknown): "valid" | "invalid" => {
  switch (currency) {
    case "KRW":
    case "USD":
      return "valid";
  }
  return "invalid";
};
const valueStatus = (
  raw: string,
  input: Exclude<NumericInput, { readonly kind: "unavailable" }>,
): "valid" | "invalid" => {
  if (!/^-?(0|[1-9]\d*)(\.\d+)?$/u.test(raw)) return "invalid";
  if (input.kind === "probability" && !/^[1-9]\d?$/u.test(raw))
    return "invalid";
  if (
    input.kind === "money" &&
    currencyStatus(input.value.currency) === "invalid"
  )
    return "invalid";
  return "valid";
};
export const formatNumber = (
  input: NumericInput,
  direction?: Direction,
): Display => {
  if (input.kind === "unavailable") return unavailable();
  const raw =
    input.kind === "decimal"
      ? input.value
      : readDecimalForNumberDisplay(input.value);
  if (typeof raw !== "string" || valueStatus(raw, input) === "invalid")
    return unavailable();
  if (directionMatches(raw, direction) === "mismatched") return unavailable();
  const prefix =
    input.kind === "money" && input.value.currency === "USD" ? "$" : "";
  return {
    kind: "available",
    text: `${sign(raw, direction)}${prefix}${magnitude(raw, input)}${unitText(input)}` as FormattedNumber,
  };
};
export const formatPriceChange = (
  amount: Money,
  rate: PercentDisplay,
  direction: Direction | undefined,
): Display => {
  if (direction === undefined) return unavailable();
  const money = formatNumber({ kind: "money", value: amount }, direction);
  const percent = formatNumber({ kind: "percent", value: rate }, direction);
  if (money.kind === "unavailable" || percent.kind === "unavailable")
    return unavailable();
  return {
    kind: "available",
    text: `${money.text}${text("number.openRate", "ko")}${percent.text}${text("number.closeRate", "ko")}` as FormattedNumber,
  };
};

export type DataDisplayInput =
  FreeText | Ticker | Timestamp | EntityId | Count | ChartDecimal | UiText;

export const formatDataValue = (value: DataDisplayInput): FormattedNumber =>
  (typeof value === "number"
    ? group(value.toString())
    : value) as FormattedNumber;

type DisplayWord = {
  readonly offset: number;
  readonly space: FormattedNumber;
  readonly word: FormattedNumber;
};
export const dataValueWords = (
  value: FormattedNumber,
): readonly DisplayWord[] =>
  Array.from(value.matchAll(/(\s*)(\S+)/gu), (match) => ({
    offset: match.index,
    space: (match[1] ?? "") as FormattedNumber,
    word: (match[2] ?? "") as FormattedNumber,
  }));

export const formatTimestampValue = (value: Timestamp): FormattedNumber =>
  (value.length >= 16 && value.charAt(10) === "T"
    ? `${value.slice(0, 10)} ${value.slice(11, 16)}`
    : value) as FormattedNumber;
