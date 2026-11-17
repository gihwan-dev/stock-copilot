import { ChartPlaceholder } from "./ChartFrame";
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
  const lines =
    props.lines === 6
      ? (["a", "b", "c", "d", "e", "f"] as const)
      : (["a", "b", "c"] as const);
  return (
    <div aria-hidden="true" className="flex flex-col gap-space-2">
      {lines.map((key): ReactElement => (
        <div
          key={key}
          className="pt-space-1 pb-space-2 rounded-sm bg-surface-sunken"
        />
      ))}
    </div>
  );
};
