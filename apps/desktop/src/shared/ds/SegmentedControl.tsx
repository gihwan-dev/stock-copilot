import { useState, type ReactElement } from "react";
import { ToggleGroup } from "@base-ui/react/toggle-group";
import { Toggle } from "@base-ui/react/toggle";
import { emptyInputValue, type UiText, type InputValue } from "../lib";
import type { Segments } from "./control-types";
type Props = {
  readonly label: UiText;
  readonly options: Segments;
  readonly value?: InputValue;
  readonly defaultValue?: InputValue;
  readonly onChange?: (value: InputValue) => void;
};
export const SegmentedControl = (props: Props): ReactElement => {
  const [inner, setInner] = useState(
    props.defaultValue ?? props.options.at(0)?.value ?? emptyInputValue,
  );
  const value = props.value ?? inner;
  return (
    <ToggleGroup
      aria-label={props.label}
      value={[value]}
      onValueChange={(values): void => {
        const next = values.at(0);
        if (next !== undefined) {
          setInner(next);
          if (props.onChange !== undefined) props.onChange(next);
        }
      }}
      className="inline-flex gap-space-1 bg-surface-sunken rounded-md p-space-1"
    >
      {props.options.map((option): ReactElement =>
        option.value === value ? (
          <Toggle
            key={option.value}
            value={option.value}
            className="bg-surface text-primary h-control-height-sm px-space-3 rounded-sm text-size-sm font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
          >
            {option.label}
          </Toggle>
        ) : (
          <Toggle
            key={option.value}
            value={option.value}
            className="bg-transparent text-ink-muted h-control-height-sm px-space-3 rounded-sm text-size-sm font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
          >
            {option.label}
          </Toggle>
        ),
      )}
    </ToggleGroup>
  );
};
