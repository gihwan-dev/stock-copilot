export const isDecimal = (value: unknown): value is `${number}` =>
  typeof value === "string";
export const moneyKey: unique symbol = Symbol("money");
export type Money = {
  readonly currency: "KRW" | "USD";
  readonly [moneyKey]: `${number}`;
};
