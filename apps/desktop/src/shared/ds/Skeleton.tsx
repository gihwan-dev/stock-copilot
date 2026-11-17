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
const variantClasses = {
  text: "flex flex-col gap-space-2",
  block: "rounded-md bg-surface-sunken h-chart-height-sm",
} as const;
const tallVariantClasses = {
  text: "flex flex-col gap-space-2",
  block: "rounded-md bg-surface-sunken h-chart-height",
} as const;
const tallerVariantClasses = {
  text: "flex flex-col gap-space-2",
  block: "rounded-md bg-surface-sunken h-chart-height-lg",
} as const;
export const Skeleton = (props: Props): ReactElement => {
  const variant = props.variant ?? "text";
  if (props.variant === "block") {
    if (props.height === "chart-height")
      return <div aria-hidden="true" className={tallVariantClasses[variant]} />;
    if (props.height === "chart-height-lg")
      return (
        <div aria-hidden="true" className={tallerVariantClasses[variant]} />
      );
    return <div aria-hidden="true" className={variantClasses[variant]} />;
  }
  const lines =
    props.lines === 6
      ? (["a", "b", "c", "d", "e", "f"] as const)
      : (["a", "b", "c"] as const);
  return (
    <div aria-hidden="true" className={variantClasses[variant]}>
      {lines.map((key): ReactElement => (
        <div key={key} className="skeleton-line rounded-sm bg-surface-sunken" />
      ))}
    </div>
  );
};
