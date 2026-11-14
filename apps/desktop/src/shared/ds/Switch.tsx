import { useState, type ReactElement } from "react";
import { Switch as Base } from "@base-ui/react/switch";
import type { UiText } from "../lib";
import type { Availability } from "./control-types";
type State = "off" | "on";
type Props = {
  readonly label: UiText;
  readonly description?: UiText;
  readonly checked?: State;
  readonly defaultChecked?: State;
  readonly disabled?: Availability;
  readonly onChange?: (state: State) => void;
};
type Control = {
  readonly props: Props;
  readonly state: State;
  readonly change: (state: State) => void;
};
const control = (data: Control): ReactElement => {
  if (data.state === "on")
    return (
      <Base.Root
        checked={true}
        disabled={data.props.disabled === "disabled"}
        aria-label={data.props.label}
        aria-description={data.props.description}
        onCheckedChange={(next): void => {
          data.change(next ? "on" : "off");
        }}
        className="inline-flex items-center w-switch h-icon-md rounded-pill justify-end border-hairline border-solid bg-primary border-primary cursor-pointer focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed"
      >
        <Base.Thumb className="size-icon-sm rounded-pill bg-on-primary" />
      </Base.Root>
    );
  return (
    <Base.Root
      checked={false}
      disabled={data.props.disabled === "disabled"}
      aria-label={data.props.label}
      aria-description={data.props.description}
      onCheckedChange={(next): void => {
        data.change(next ? "on" : "off");
      }}
      className="inline-flex items-center w-switch h-icon-md rounded-pill justify-start border-hairline border-solid bg-surface-sunken border-line-strong cursor-pointer focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed"
    >
      <Base.Thumb className="size-icon-sm rounded-pill bg-ink-muted" />
    </Base.Root>
  );
};
export const Switch = (props: Props): ReactElement => {
  const [inner, setInner] = useState<State>(props.defaultChecked ?? "off");
  const state = props.checked ?? inner;
  const change = (next: State): void => {
    if (props.disabled !== "disabled") {
      setInner(next);
      if (props.onChange !== undefined) props.onChange(next);
    }
  };
  return (
    <label className="flex items-start gap-space-3 cursor-pointer text-ink">
      <span>{control({ props, state, change })}</span>
      <span className="flex flex-col gap-space-1">
        <span className="text-label">{props.label}</span>
        <span className="text-caption text-ink-subtle">
          {props.description}
        </span>
      </span>
    </label>
  );
};
