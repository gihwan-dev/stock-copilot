export const run = (target: Readonly<{ a?: "x" }>): unknown =>
  Object.defineProperties(target, {});
