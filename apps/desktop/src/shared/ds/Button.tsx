import type { ReactElement } from "react";
import type { ButtonProps } from "./control-types";
import { primary, secondary, ghost } from "./button-variants";
export const Button = (props: ButtonProps): ReactElement => {
  if (props.variant === "primary") return primary(props);
  if (props.variant === "ghost") return ghost(props);
  return secondary(props);
};
