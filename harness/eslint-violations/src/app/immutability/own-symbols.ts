export const keys = (target: Readonly<{ a: "x" }>): unknown =>
  Object.getOwnPropertySymbols(target);
