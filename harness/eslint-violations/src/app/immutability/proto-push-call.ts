export const run = (values: readonly "a"[]): unknown =>
  Array.prototype.push.call(values, "a");
