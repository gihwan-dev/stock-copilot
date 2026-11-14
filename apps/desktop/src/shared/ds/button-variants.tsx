import type { ReactElement } from "react";
import { Button as BaseButton } from "@base-ui/react/button";
import type { ButtonProps } from "./control-types";
export const primary = (props: ButtonProps): ReactElement => {
  if (props.size === "control-height-sm")
    return (
      <BaseButton
        type={props.type ?? "button"}
        disabled={props.disabled === "disabled"}
        aria-label={props["aria-label"]}
        onClick={(): void => {
          if (props.onClick !== undefined) props.onClick();
        }}
        className="inline-flex items-center justify-center gap-space-1 rounded-md border-hairline border-solid border-transparent cursor-pointer font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed bg-primary text-on-primary hover:bg-primary-hover disabled:bg-primary h-control-height-sm px-control-pad-sm text-size-sm"
      >
        {props.children}
      </BaseButton>
    );
  return (
    <BaseButton
      type={props.type ?? "button"}
      disabled={props.disabled === "disabled"}
      aria-label={props["aria-label"]}
      onClick={(): void => {
        if (props.onClick !== undefined) props.onClick();
      }}
      className="inline-flex items-center justify-center gap-space-1 rounded-md border-hairline border-solid border-transparent cursor-pointer font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed bg-primary text-on-primary hover:bg-primary-hover disabled:bg-primary h-control-height px-space-3 text-size-md"
    >
      {props.children}
    </BaseButton>
  );
};
export const secondary = (props: ButtonProps): ReactElement => {
  if (props.size === "control-height-sm")
    return (
      <BaseButton
        type={props.type ?? "button"}
        disabled={props.disabled === "disabled"}
        aria-label={props["aria-label"]}
        onClick={(): void => {
          if (props.onClick !== undefined) props.onClick();
        }}
        className="inline-flex items-center justify-center gap-space-1 rounded-md border-hairline border-solid cursor-pointer font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed bg-surface text-ink border-line-strong hover:bg-surface-sunken disabled:bg-surface h-control-height-sm px-control-pad-sm text-size-sm"
      >
        {props.children}
      </BaseButton>
    );
  return (
    <BaseButton
      type={props.type ?? "button"}
      disabled={props.disabled === "disabled"}
      aria-label={props["aria-label"]}
      onClick={(): void => {
        if (props.onClick !== undefined) props.onClick();
      }}
      className="inline-flex items-center justify-center gap-space-1 rounded-md border-hairline border-solid cursor-pointer font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed bg-surface text-ink border-line-strong hover:bg-surface-sunken disabled:bg-surface h-control-height px-space-3 text-size-md"
    >
      {props.children}
    </BaseButton>
  );
};
export const ghost = (props: ButtonProps): ReactElement => {
  if (props.size === "control-height-sm")
    return (
      <BaseButton
        type={props.type ?? "button"}
        disabled={props.disabled === "disabled"}
        aria-label={props["aria-label"]}
        onClick={(): void => {
          if (props.onClick !== undefined) props.onClick();
        }}
        className="inline-flex items-center justify-center gap-space-1 rounded-md border-hairline border-solid border-transparent cursor-pointer font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed bg-transparent text-ink-muted hover:bg-surface-sunken disabled:bg-transparent h-control-height-sm px-control-pad-sm text-size-sm"
      >
        {props.children}
      </BaseButton>
    );
  return (
    <BaseButton
      type={props.type ?? "button"}
      disabled={props.disabled === "disabled"}
      aria-label={props["aria-label"]}
      onClick={(): void => {
        if (props.onClick !== undefined) props.onClick();
      }}
      className="inline-flex items-center justify-center gap-space-1 rounded-md border-hairline border-solid border-transparent cursor-pointer font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed bg-transparent text-ink-muted hover:bg-surface-sunken disabled:bg-transparent h-control-height px-space-3 text-size-md"
    >
      {props.children}
    </BaseButton>
  );
};
