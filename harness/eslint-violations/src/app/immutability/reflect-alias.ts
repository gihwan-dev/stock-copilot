const reflect = Reflect;
export const keys = (target: Readonly<{ a: "x" }>): unknown =>
  reflect.ownKeys(target);
