import { useId, useState, type ReactElement } from "react";
import { inputEventValue } from "../platform/input";
import { emptyInputValue, type UiText, type InputValue } from "../lib";
import type { Option, Availability } from "./control-types";
type Props = {
  readonly label: UiText;
  readonly options: readonly Option[];
  readonly value?: InputValue;
  readonly defaultValue?: InputValue;
  readonly onChange?: (value: InputValue) => void;
  readonly hint?: UiText;
  readonly error?: UiText;
  readonly disabled?: Availability;
};
const initialValue = (props: Props): InputValue =>
  props.defaultValue ?? props.options.at(0)?.value ?? emptyInputValue;
export const Select = (props: Props): ReactElement => {
  const id = useId();
  const inputId = useId();
  const [inner, setInner] = useState((): InputValue => initialValue(props));
  const value = props.value ?? inner;
  const changed = (next: InputValue): void => {
    if (props.disabled !== "disabled") {
      setInner(next);
      if (props.onChange !== undefined) props.onChange(next);
    }
  };
  return (
    <div className="flex flex-col gap-space-1 text-ink">
      <label htmlFor={inputId} className="text-label">
        {props.label}
      </label>
      <select
        id={inputId}
        value={value}
        disabled={props.disabled === "disabled"}
        aria-invalid={props.error !== undefined}
        aria-describedby={
          props.error === undefined && props.hint === undefined ? undefined : id
        }
        onChange={(event): void => {
          const next = inputEventValue(event);
          if (next.kind === "ok") changed(next.value);
        }}
        className={
          props.error !== undefined
            ? "h-control-height w-full bg-surface-sunken border-hairline border-solid border-alert rounded-md px-space-3 text-body disabled:opacity-disabled focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
            : "h-control-height w-full bg-surface-sunken border-hairline border-solid border-line-strong rounded-md px-space-3 text-body disabled:opacity-disabled focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
        }
      >
        {props.options.map((option): ReactElement => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
      {props.error !== undefined ? (
        <span id={id} role="alert" className="text-caption text-alert">
          {props.error}
        </span>
      ) : (
        <span id={id} className="text-caption text-ink-subtle">
          {props.hint}
        </span>
      )}
    </div>
  );
};
