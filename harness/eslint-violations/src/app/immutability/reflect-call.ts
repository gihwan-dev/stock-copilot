export const keys = (target: Readonly<{ a: "x" }>): unknown =>
  Reflect.ownKeys(target);
