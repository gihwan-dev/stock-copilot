import { useId, useState, type ReactElement } from "react";
import { Button as BaseButton } from "@base-ui/react/button";
import { Tooltip } from "@base-ui/react/tooltip";
import type { UiText } from "../lib";
import type { IconName } from "./icon-paths";
import type { ControlSize, Availability } from "./control-types";
import { Icon } from "./Icon";
type Props = {
  readonly icon: IconName;
  readonly label: UiText;
  readonly onClick?: () => void;
  readonly size?: ControlSize;
  readonly pressed?: "pressed" | "released";
  readonly expanded?: "open" | "closed";
  readonly disabled?: Availability;
  readonly tooltip?: "shown" | "hidden";
  readonly tooltipSide?: "top" | "right";
};
const small = (props: Props): ReactElement => (
  <BaseButton
    type="button"
    aria-label={props.label}
    aria-pressed={
      props.pressed === undefined ? undefined : props.pressed === "pressed"
    }
    aria-expanded={
      props.expanded === undefined ? undefined : props.expanded === "open"
    }
    disabled={props.disabled === "disabled"}
    onClick={(): void => {
      if (props.onClick !== undefined) props.onClick();
    }}
    className="inline-grid place-items-center flex-none rounded-md cursor-pointer bg-transparent text-ink-muted hover:bg-surface-sunken hover:text-ink disabled:opacity-disabled disabled:cursor-not-allowed focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus size-control-height-sm"
  >
    {props.pressed === "pressed" ? (
      <span className="bg-primary-soft text-primary rounded-md">
        <Icon name={props.icon} size="icon-md" />
      </span>
    ) : (
      <Icon name={props.icon} size="icon-md" />
    )}
  </BaseButton>
);
const normal = (props: Props): ReactElement => (
  <BaseButton
    type="button"
    aria-label={props.label}
    aria-pressed={
      props.pressed === undefined ? undefined : props.pressed === "pressed"
    }
    aria-expanded={
      props.expanded === undefined ? undefined : props.expanded === "open"
    }
    disabled={props.disabled === "disabled"}
    onClick={(): void => {
      if (props.onClick !== undefined) props.onClick();
    }}
    className="inline-grid place-items-center flex-none rounded-md cursor-pointer bg-transparent text-ink-muted hover:bg-surface-sunken hover:text-ink disabled:opacity-disabled disabled:cursor-not-allowed focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:outline-offset-focus size-control-height"
  >
    {props.pressed === "pressed" ? (
      <span className="bg-primary-soft text-primary rounded-md">
        <Icon name={props.icon} size="icon-md" />
      </span>
    ) : (
      <Icon name={props.icon} size="icon-md" />
    )}
  </BaseButton>
);

export const IconButton = (props: Props): ReactElement => {
  const id = useId();
  const [tooltipState, setTooltipState] = useState<"open" | "closed">("closed");
  const button =
    props.size === "control-height-sm" ? small(props) : normal(props);
  if (props.tooltip === "hidden") return button;
  return (
    <Tooltip.Provider>
      <Tooltip.Root
        onOpenChange={(open): void => {
          setTooltipState(open ? "open" : "closed");
        }}
      >
        <Tooltip.Trigger
          render={button}
          aria-describedby={tooltipState === "open" ? id : undefined}
        />
        <Tooltip.Portal>
          <Tooltip.Positioner
            side={props.tooltipSide ?? "top"}
            className="z-popover"
          >
            <Tooltip.Popup
              id={id}
              role="tooltip"
              className="bg-ink text-surface rounded-sm px-space-2 py-space-1 text-caption max-w-tooltip-max-width"
            >
              {props.label}
            </Tooltip.Popup>
          </Tooltip.Positioner>
        </Tooltip.Portal>
      </Tooltip.Root>
    </Tooltip.Provider>
  );
};
