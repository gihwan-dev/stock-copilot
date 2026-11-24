import type { ReactElement } from "react";

type Direction = "horizontal" | "vertical";

type Props = {
  readonly direction?: Direction;
};

export const Divider = (props: Props): ReactElement => {
  if (props.direction === "vertical")
    return (
      <div
        role="separator"
        aria-orientation="vertical"
        className="w-border-hairline self-stretch min-h-control-height-sm bg-line"
      />
    );
  return (
    <div
      role="separator"
      aria-orientation="horizontal"
      className="w-full max-w-full h-border-hairline bg-line"
    />
  );
};
