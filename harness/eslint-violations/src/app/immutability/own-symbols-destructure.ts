const { getOwnPropertySymbols } = Object;
export const keys = (target: Readonly<{ a: "x" }>): unknown =>
  getOwnPropertySymbols(target);
