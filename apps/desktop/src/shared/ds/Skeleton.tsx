import { ChartPlaceholder, ChartLoadingLines } from "./chart-loading";
import type { ReactElement } from "react";
import type { SizeToken } from "./tokens";
type Props =
  | { readonly variant?: "text"; readonly lines?: 3 | 6 }
  | {
      readonly variant: "block";
      readonly height?: Extract<
        SizeToken,
        "chart-height-sm" | "chart-height" | "chart-height-lg"
      >;
    };
export const Skeleton = (props: Props): ReactElement => {
  if (props.variant === "block")
    return <ChartPlaceholder height={props.height ?? "chart-height-sm"} />;
  return <ChartLoadingLines lines={props.lines ?? 3} />;
};
