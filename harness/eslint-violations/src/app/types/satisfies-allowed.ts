type Labels = Readonly<Record<"buy" | "sell", "매수" | "매도">>;
export const labels = { buy: "매수", sell: "매도" } as const satisfies Labels;
