const key = "length";
export const size = (items: ReadonlySet<"a">): "x" =>
  items[key] === 0 ? "x" : "x";
