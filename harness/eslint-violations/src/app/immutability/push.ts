export const run = (values: readonly "a"[]): readonly "a"[] => {
  const copy: "a"[] = [...values];
  copy.push("a");
  return copy;
};
