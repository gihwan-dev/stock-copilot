const labels = { buy: "b", sell: "s" } as const;
export const label = (side: "buy" | "sell"): "b" | "s" => labels[side];
export const first = (items: readonly ("a" | "b")[]): "a" | "b" | undefined =>
  items[0];
