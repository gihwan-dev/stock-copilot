import { Popover as BasePopover } from "@base-ui/react/popover";
import type { ReactElement } from "react";
import type { UiText } from "../lib";

export function Popover(props: {
  readonly label: UiText;
  readonly trigger: ReactElement;
  readonly children: ReactElement;
  readonly visibility: "open" | "closed";
  readonly onChange: (value: "open" | "closed") => void;
}): ReactElement {
  return (
    <BasePopover.Root
      open={props.visibility === "open"}
      onOpenChange={(open): void => {
        props.onChange(open ? "open" : "closed");
      }}
    >
      <BasePopover.Trigger render={props.trigger} />
      <BasePopover.Portal>
        <BasePopover.Positioner side="bottom" align="end" className="z-popover">
          <BasePopover.Popup
            aria-label={props.label}
            className="flex flex-col p-space-3 bg-surface-raised rounded-md shadow-popover text-body-sm text-ink"
          >
            {props.children}
          </BasePopover.Popup>
        </BasePopover.Positioner>
      </BasePopover.Portal>
    </BasePopover.Root>
  );
}
