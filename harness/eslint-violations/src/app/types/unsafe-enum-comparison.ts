enum Side {
  Buy = "buy",
}
export const isBuy = (side: Side): "yes" | "no" =>
  side === "buy" ? "yes" : "no";
