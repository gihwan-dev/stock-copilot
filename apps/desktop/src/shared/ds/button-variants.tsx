import type { ReactElement } from "react";
import { Button as BaseButton } from "@base-ui/react/button";
import type { ButtonProps } from "./control-types";
const smallVariantClasses = {
  primary:
    "inline-flex items-center justify-center gap-space-1 rounded-md border-hairline border-solid border-transparent cursor-pointer font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed bg-primary text-on-primary hover:bg-primary-hover disabled:bg-primary h-control-height-sm px-control-pad-sm text-size-sm",
  secondary:
    "inline-flex items-center justify-center gap-space-1 rounded-md border-hairline border-solid cursor-pointer font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed bg-surface text-ink border-line-strong hover:bg-surface-sunken disabled:bg-surface h-control-height-sm px-control-pad-sm text-size-sm",
  ghost:
    "inline-flex items-center justify-center gap-space-1 rounded-md border-hairline border-solid border-transparent cursor-pointer font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed bg-transparent text-ink-muted hover:bg-surface-sunken disabled:bg-transparent h-control-height-sm px-control-pad-sm text-size-sm",
} as const;
const normalVariantClasses = {
  primary:
    "inline-flex items-center justify-center gap-space-1 rounded-md border-hairline border-solid border-transparent cursor-pointer font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed bg-primary text-on-primary hover:bg-primary-hover disabled:bg-primary h-control-height px-space-3 text-size-md",
  secondary:
    "inline-flex items-center justify-center gap-space-1 rounded-md border-hairline border-solid cursor-pointer font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed bg-surface text-ink border-line-strong hover:bg-surface-sunken disabled:bg-surface h-control-height px-space-3 text-size-md",
  ghost:
    "inline-flex items-center justify-center gap-space-1 rounded-md border-hairline border-solid border-transparent cursor-pointer font-semibold focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus disabled:opacity-disabled disabled:cursor-not-allowed bg-transparent text-ink-muted hover:bg-surface-sunken disabled:bg-transparent h-control-height px-space-3 text-size-md",
} as const;
export const buttonVariant = (props: ButtonProps): ReactElement => {
  const variant = props.variant ?? "secondary";
  if (props.size === "control-height-sm")
    return (
      <BaseButton
        type={props.type ?? "button"}
        disabled={props.disabled === "disabled"}
        aria-label={props["aria-label"]}
        onClick={(): void => {
          if (props.onClick !== undefined) props.onClick();
        }}
        className={smallVariantClasses[variant]}
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
      className={normalVariantClasses[variant]}
    >
      {props.children}
    </BaseButton>
  );
};
