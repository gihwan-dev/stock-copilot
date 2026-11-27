import { Tooltip } from "@base-ui/react/tooltip";
import type { ReactElement } from "react";
import type { FreeText } from "../lib";
import { DataText } from "./Numbers";

export const CitationTooltip = (props: {
  readonly detail: FreeText;
  readonly trigger: ReactElement;
}): ReactElement => (
  <Tooltip.Provider>
    <Tooltip.Root>
      <Tooltip.Trigger render={props.trigger} />
      <Tooltip.Portal>
        <Tooltip.Positioner side="top" className="z-modal">
          <Tooltip.Popup
            role="tooltip"
            className="bg-surface-raised text-ink rounded-sm shadow-popover px-space-2 py-space-1 max-w-full"
          >
            <DataText value={props.detail} variant="caption" />
          </Tooltip.Popup>
        </Tooltip.Positioner>
      </Tooltip.Portal>
    </Tooltip.Root>
  </Tooltip.Provider>
);
