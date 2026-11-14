import type { UiText, InputValue } from "../lib";
import type { SizeToken } from "./tokens";
export type ControlSize = Extract<
  SizeToken,
  "control-height" | "control-height-sm"
>;
export type Availability = "enabled" | "disabled";
export type Option = { readonly value: InputValue; readonly label: UiText };
export type Segments =
  | readonly [Option, Option]
  | readonly [Option, Option, Option]
  | readonly [Option, Option, Option, Option]
  | readonly [Option, Option, Option, Option, Option];
export type ButtonProps = {
  readonly children: UiText;
  readonly variant?: "primary" | "secondary" | "ghost";
  readonly size?: ControlSize;
  readonly disabled?: Availability;
  readonly type?: "button" | "submit";
  readonly onClick?: () => void;
  readonly "aria-label"?: UiText;
};
