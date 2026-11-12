export const isDecimal = (value: unknown): value is `${number}` =>
  typeof value === "string";
