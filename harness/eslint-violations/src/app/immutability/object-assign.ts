export const merge = (target: Readonly<{ a?: "x" }>): Readonly<{ a?: "x" }> =>
  Object.assign({}, target, { a: "x" });
