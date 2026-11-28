import type { ReactElement } from "react";
import { Separator, type Orientation } from "react-resizable-panels";
import { Divider } from "../Divider";
import { useWidgetAppearance } from "../AppFrame";
import { dockText } from "./messages";
const horizontal = {
  "0": (
    <Separator
      data-dock-divider="horizontal"
      aria-label={dockText.split}
      className="flex flex-none bg-line hover:bg-primary focus-visible:bg-primary"
    >
      <Divider direction="vertical" />
    </Separator>
  ),
  "4": (
    <Separator
      data-dock-divider="horizontal"
      aria-label={dockText.split}
      className="pl-space-1 flex flex-none bg-line hover:bg-primary focus-visible:bg-primary"
    />
  ),
  "8": (
    <Separator
      data-dock-divider="horizontal"
      aria-label={dockText.split}
      className="pl-space-2 flex flex-none bg-line hover:bg-primary focus-visible:bg-primary"
    />
  ),
  "12": (
    <Separator
      data-dock-divider="horizontal"
      aria-label={dockText.split}
      className="pl-space-3 flex flex-none bg-line hover:bg-primary focus-visible:bg-primary"
    />
  ),
} as const;
const vertical = {
  "0": (
    <Separator
      data-dock-divider="vertical"
      aria-label={dockText.split}
      className="flex flex-none bg-line hover:bg-primary focus-visible:bg-primary"
    >
      <Divider direction="horizontal" />
    </Separator>
  ),
  "4": (
    <Separator
      data-dock-divider="vertical"
      aria-label={dockText.split}
      className="pt-space-1 flex flex-none bg-line hover:bg-primary focus-visible:bg-primary"
    />
  ),
  "8": (
    <Separator
      data-dock-divider="vertical"
      aria-label={dockText.split}
      className="pt-space-2 flex flex-none bg-line hover:bg-primary focus-visible:bg-primary"
    />
  ),
  "12": (
    <Separator
      data-dock-divider="vertical"
      aria-label={dockText.split}
      className="pt-space-3 flex flex-none bg-line hover:bg-primary focus-visible:bg-primary"
    />
  ),
} as const;
export function DockDivider({
  axis,
}: {
  readonly axis: Orientation;
}): ReactElement {
  const { gap } = useWidgetAppearance();
  return axis === "horizontal" ? horizontal[gap] : vertical[gap];
}
