enum Side {
  Buy = 1,
}
export const check = (value: Side | undefined): "yes" | "no" =>
  value ? "yes" : "no";
