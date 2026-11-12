import type { ComponentProps, ReactElement } from "react";
export const Field = (
  props: Readonly<ComponentProps<"input">>,
): ReactElement => <input key={props.id} />;
