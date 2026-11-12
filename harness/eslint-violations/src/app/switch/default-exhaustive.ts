export const label = (side: "buy" | "sell"): "매수" | "매도" => {
  switch (side) {
    case "buy":
      return "매수";
    case "sell":
      return "매도";
    default:
      return "매도";
  }
};
