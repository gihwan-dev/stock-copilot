export const run = (values: readonly "a"[]): unknown =>
  [].push.call(values, "a");
