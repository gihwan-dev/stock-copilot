import { useId, useState, type ReactElement } from "react";
import { Input } from "@base-ui/react/input";
import { inputEventValue, inputCommit, pasteInputValue } from "../platform";
import {
  emptyInputValue,
  parseEntityId,
  text,
  type InputValue,
  type UiText,
  type EntityId,
} from "../lib";
import type { Availability } from "./control-types";
import { IconButton } from "./IconButton";
import { Button } from "./Button";
type Props = {
  readonly label: UiText;
  readonly value?: InputValue;
  readonly defaultValue?: InputValue;
  readonly onChange?: (value: InputValue) => void;
  readonly onCommit?: (value: InputValue) => void;
  readonly placeholder?: UiText;
  readonly hint?: UiText;
  readonly error?: UiText;
  readonly disabled?: Availability;
  readonly type?: "text" | "secret";
};
type Visibility = "masked" | "visible";
type FieldState = {
  readonly value: InputValue;
  readonly error: UiText | undefined;
  readonly update: (value: InputValue) => void;
  readonly paste: () => void;
};
const useFieldState = (props: Props): FieldState => {
  const [inner, setInner] = useState(props.defaultValue ?? emptyInputValue);
  const [pasteState, setPasteState] = useState<"ready" | "failed">("ready");
  const update = (next: InputValue): void => {
    if (props.disabled !== "disabled") {
      setInner(next);
      setPasteState("ready");
      if (props.onChange !== undefined) props.onChange(next);
    }
  };
  const paste = (): void => {
    if (props.disabled !== "disabled")
      pasteInputValue((result): void => {
        if (result.kind === "ok") update(result.value);
        else setPasteState("failed");
      });
  };
  return {
    value: props.value ?? inner,
    error:
      props.error ??
      (pasteState === "failed" ? text("ds.pasteError", "ko") : undefined),
    update,
    paste,
  };
};
type InputProps = {
  readonly field: Props;
  readonly state: FieldState;
  readonly visibility: Visibility;
  readonly descriptionId: EntityId;
};
const TextInput = (data: InputProps): ReactElement => {
  const [composition, setComposition] = useState<"idle" | "composing">("idle");
  return (
    <Input
      type={
        data.field.type === "secret" && data.visibility === "masked"
          ? "password"
          : "text"
      }
      value={data.state.value}
      aria-label={data.field.label}
      onChange={(event): void => {
        const next = inputEventValue(event);
        if (next.kind === "ok") data.state.update(next.value);
      }}
      onCompositionStart={(): void => {
        setComposition("composing");
      }}
      onCompositionEnd={(): void => {
        setComposition("idle");
      }}
      onKeyDown={(event): void => {
        if (
          composition === "idle" &&
          inputCommit(event) === "commit" &&
          data.field.onCommit !== undefined &&
          data.field.disabled !== "disabled"
        )
          data.field.onCommit(data.state.value);
      }}
      placeholder={data.field.placeholder}
      disabled={data.field.disabled === "disabled"}
      autoComplete={data.field.type === "secret" ? "off" : undefined}
      spellCheck={data.field.type === "secret" ? false : undefined}
      aria-invalid={data.state.error !== undefined}
      aria-describedby={
        data.state.error === undefined && data.field.hint === undefined
          ? undefined
          : data.descriptionId
      }
      className={
        data.state.error !== undefined
          ? "h-control-height w-full px-space-3 text-body bg-surface-sunken border-hairline border-solid border-alert rounded-md disabled:opacity-disabled focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
          : "h-control-height w-full px-space-3 text-body bg-surface-sunken border-hairline border-solid border-line-strong rounded-md disabled:opacity-disabled focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus"
      }
    />
  );
};
type RevealProps = {
  readonly field: Props;
  readonly state: FieldState;
  readonly visibility: Visibility;
  readonly toggle: () => void;
};
const reveal = (data: RevealProps): ReactElement | null => {
  if (data.field.type !== "secret" || data.state.value === emptyInputValue)
    return null;
  return (
    <IconButton
      icon={data.visibility === "masked" ? "eye" : "eye-off"}
      label={text(data.visibility === "masked" ? "ds.show" : "ds.hide", "ko")}
      size="control-height-sm"
      disabled={data.field.disabled ?? "enabled"}
      onClick={data.toggle}
    />
  );
};
export const TextField = (props: Props): ReactElement | null => {
  const state = useFieldState(props);
  const [visibility, setVisibility] = useState<Visibility>("masked");
  const id = parseEntityId(useId());
  if (id.kind === "err") return null;
  const toggle = (): void => {
    setVisibility(visibility === "masked" ? "visible" : "masked");
  };
  return (
    <label className="flex flex-col gap-space-1 text-ink">
      <span className="text-label">{props.label}</span>
      <span className="flex items-center gap-space-1 bg-surface-sunken rounded-md">
        <TextInput
          field={props}
          state={state}
          visibility={visibility}
          descriptionId={id.value}
        />
        {reveal({ field: props, state, visibility, toggle })}
        {props.type === "secret" ? (
          <Button
            variant="ghost"
            size="control-height-sm"
            disabled={props.disabled ?? "enabled"}
            onClick={state.paste}
          >
            {text("ds.paste", "ko")}
          </Button>
        ) : null}
      </span>
      {state.error !== undefined ? (
        <span id={id.value} role="alert" className="text-caption text-alert">
          {state.error}
        </span>
      ) : (
        <span id={id.value} className="text-caption text-ink-subtle">
          {props.hint}
        </span>
      )}
    </label>
  );
};
