export const run = (source: ReadonlyMap<"a", "b">): ReadonlyMap<"a", "b"> =>
  new Map(source).set("a", "b");
