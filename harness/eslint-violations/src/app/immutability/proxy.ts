export const wrap = (target: Readonly<{ a: "x" }>): unknown =>
  new Proxy(target, {});
