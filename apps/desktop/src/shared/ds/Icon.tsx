import type { ReactElement } from "react";
import type { UiText } from "../lib";
import type { SizeToken } from "./tokens";
import { IconPaths, type IconName } from "./icon-paths";
type Props = {
  readonly name: IconName;
  readonly size?: Extract<SizeToken, "icon-sm" | "icon-md" | "icon-lg">;
  readonly label?: UiText;
};
const sm = (props: Props): ReactElement => {
  if (props.label !== undefined)
    return (
      <svg
        viewBox="0 0 24 24"
        focusable="false"
        role="img"
        aria-label={props.label}
        className="inline-block flex-none size-icon-sm fill-none stroke-current stroke-icon stroke-linecap-round stroke-linejoin-round"
      >
        <IconPaths name={props.name} />
      </svg>
    );
  return (
    <svg
      viewBox="0 0 24 24"
      focusable="false"
      aria-hidden="true"
      className="inline-block flex-none size-icon-sm fill-none stroke-current stroke-icon stroke-linecap-round stroke-linejoin-round"
    >
      <IconPaths name={props.name} />
    </svg>
  );
};
const md = (props: Props): ReactElement => {
  if (props.label !== undefined)
    return (
      <svg
        viewBox="0 0 24 24"
        focusable="false"
        role="img"
        aria-label={props.label}
        className="inline-block flex-none size-icon-md fill-none stroke-current stroke-icon stroke-linecap-round stroke-linejoin-round"
      >
        <IconPaths name={props.name} />
      </svg>
    );
  return (
    <svg
      viewBox="0 0 24 24"
      focusable="false"
      aria-hidden="true"
      className="inline-block flex-none size-icon-md fill-none stroke-current stroke-icon stroke-linecap-round stroke-linejoin-round"
    >
      <IconPaths name={props.name} />
    </svg>
  );
};
const lg = (props: Props): ReactElement => {
  if (props.label !== undefined)
    return (
      <svg
        viewBox="0 0 24 24"
        focusable="false"
        role="img"
        aria-label={props.label}
        className="inline-block flex-none size-icon-lg fill-none stroke-current stroke-icon stroke-linecap-round stroke-linejoin-round"
      >
        <IconPaths name={props.name} />
      </svg>
    );
  return (
    <svg
      viewBox="0 0 24 24"
      focusable="false"
      aria-hidden="true"
      className="inline-block flex-none size-icon-lg fill-none stroke-current stroke-icon stroke-linecap-round stroke-linejoin-round"
    >
      <IconPaths name={props.name} />
    </svg>
  );
};
export const Icon = (props: Props): ReactElement => {
  if (props.size === "icon-md") return md(props);
  if (props.size === "icon-lg") return lg(props);
  return sm(props);
};
