import type { ReactElement } from "react";
import type { ButtonProps } from "./control-types";
import { primary, secondary, ghost, buy, sell } from "./button-variants";
export const Button = (props: ButtonProps): ReactElement => {
  if (props.variant === "primary") return primary(props);
  if (props.variant === "ghost") return ghost(props);
  if (props.variant === "buy") return buy(props);
  if (props.variant === "sell") return sell(props);
  return secondary(props);
};
