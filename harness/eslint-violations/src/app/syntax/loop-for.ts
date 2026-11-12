export const run = (values: readonly "a"[]): "a" | "b" => {
  for (const value of values) {
    return value;
  }
  return "b";
};
