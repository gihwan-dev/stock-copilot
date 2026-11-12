export const label = (side: "buy" | "sell"): "매수" | "기타" => {
  switch (side) {
    case "buy":
      return "매수";
    default:
      return "기타";
  }
};
