import { useState, type ReactElement } from "react";
import { Checkbox as Base } from "@base-ui/react/checkbox";
import type { UiText } from "../lib";
import type { Availability } from "./control-types";
import { Icon } from "./Icon";
type State = "unchecked" | "checked";
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
  if (data.state === "checked")
    return (
      <Base.Root
        render={<button type="button" />}
        nativeButton
        checked={true}
        disabled={data.props.disabled === "disabled"}
        aria-label={data.props.label}
        aria-description={data.props.description}
        onCheckedChange={(next): void => {
          data.change(next ? "checked" : "unchecked");
        }}
        className="inline-grid place-items-center size-icon-sm rounded-sm border-hairline border-solid bg-primary border-primary cursor-pointer focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed"
      >
        <Base.Indicator>
          <Icon name="check" />
        </Base.Indicator>
      </Base.Root>
    );
  return (
    <Base.Root
      render={<button type="button" />}
      nativeButton
      checked={false}
      disabled={data.props.disabled === "disabled"}
      aria-label={data.props.label}
      aria-description={data.props.description}
      onCheckedChange={(next): void => {
        data.change(next ? "checked" : "unchecked");
      }}
      className="inline-grid place-items-center size-icon-sm rounded-sm border-hairline border-solid bg-surface border-line-strong cursor-pointer focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed"
    >
      <Base.Indicator>
        <Icon name="check" />
      </Base.Indicator>
    </Base.Root>
  );
};
export const Checkbox = (props: Props): ReactElement => {
  const [inner, setInner] = useState<State>(
    props.defaultChecked ?? "unchecked",
  );
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
