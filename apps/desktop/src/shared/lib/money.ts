import { err, ok, type Result } from "./result";

const moneyKey: unique symbol = Symbol("money");
const quantityKey: unique symbol = Symbol("quantity");
const priceKey: unique symbol = Symbol("price");
const ratioKey: unique symbol = Symbol("ratio");
const percentDisplayKey: unique symbol = Symbol("percent-display");
const probabilityKey: unique symbol = Symbol("probability");

export type Currency = "KRW" | "USD";

type Decimal = `${number}`;

export type Money = {
  readonly currency: Currency;
  readonly [moneyKey]: Decimal;
};

export type Price = {
  readonly currency: Currency;
  readonly [priceKey]: Decimal;
};

export type Quantity = { readonly [quantityKey]: Decimal };

export type Ratio = { readonly [ratioKey]: Decimal };

export type PercentDisplay = { readonly [percentDisplayKey]: Decimal };

export type Probability = { readonly [probabilityKey]: Decimal };

export type DecimalParseError =
  | { readonly kind: "malformed-decimal" }
  | { readonly kind: "unknown-currency" };

const decimalPattern = /^-?(0|[1-9]\d*)(\.\d+)?$/u;

const isDecimal = (input: unknown): input is Decimal =>
  typeof input === "string" && decimalPattern.test(input);

const parseDecimal = (input: unknown): Result<Decimal, DecimalParseError> =>
  isDecimal(input) ? ok(input) : err({ kind: "malformed-decimal" });

const parseCurrency = (input: string): Result<Currency, DecimalParseError> => {
  switch (input) {
    case "KRW":
      return ok("KRW");
    case "USD":
      return ok("USD");
  }
  return err({ kind: "unknown-currency" });
};

export const parseMoney = (
  amount: string,
  currency: string,
): Result<Money, DecimalParseError> => {
  const decimal = parseDecimal(amount);
  const unit = parseCurrency(currency);
  if (decimal.kind === "err") return decimal;
  if (unit.kind === "err") return unit;
  return ok({ currency: unit.value, [moneyKey]: decimal.value });
};

export const parsePrice = (
  amount: string,
  currency: string,
): Result<Price, DecimalParseError> => {
  const decimal = parseDecimal(amount);
  const unit = parseCurrency(currency);
  if (decimal.kind === "err") return decimal;
  if (unit.kind === "err") return unit;
  return ok({ currency: unit.value, [priceKey]: decimal.value });
};

const mapDecimal = <T>(
  input: string,
  wrap: (decimal: Decimal) => T,
): Result<T, DecimalParseError> => {
  const decimal = parseDecimal(input);
  return decimal.kind === "ok" ? ok(wrap(decimal.value)) : decimal;
};

export const parseQuantity = (
  input: string,
): Result<Quantity, DecimalParseError> =>
  mapDecimal(input, (decimal) => ({ [quantityKey]: decimal }));

export const parseRatio = (input: string): Result<Ratio, DecimalParseError> =>
  mapDecimal(input, (decimal) => ({ [ratioKey]: decimal }));

export const parseProbability = (
  input: string,
): Result<Probability, DecimalParseError> =>
  mapDecimal(input, (decimal) => ({ [probabilityKey]: decimal }));

export const parsePercentDisplay = (
  input: string,
): Result<PercentDisplay, DecimalParseError> =>
  mapDecimal(input, (decimal) => ({ [percentDisplayKey]: decimal }));

export const readDecimalForNumberDisplay = (
  value: Money | Price | Quantity | PercentDisplay | null | undefined,
): Decimal | undefined => {
  if (typeof value !== "object" || value === null) return undefined;
  if (moneyKey in value) return value[moneyKey];
  if (priceKey in value) return value[priceKey];
  if (quantityKey in value) return value[quantityKey];
  if (percentDisplayKey in value) return value[percentDisplayKey];
  return undefined;
};
