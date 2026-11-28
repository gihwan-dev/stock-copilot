import type { ReactElement } from "react";

export type NumberEmphasis = "inherit" | "display" | "large" | "value";
export const NumberSize = (props: {
  readonly emphasis: NumberEmphasis | undefined;
  readonly children: ReactElement;
}): ReactElement => {
  switch (props.emphasis) {
    case "display":
      return <span className="text-num-display">{props.children}</span>;
    case "value":
      return <span className="text-title-sm text-ink">{props.children}</span>;
    case "large":
      return <span className="text-num-lg">{props.children}</span>;
    case "inherit":
    case undefined:
      return props.children;
  }
};
