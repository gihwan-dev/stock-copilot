import { createContext, use, type ReactElement } from "react";
import type { DockPanel, UiText } from "../lib";
import { useWidgetAppearance } from "./AppFrame";

const frameClasses = {
  "0": "flex flex-col h-full w-full bg-surface",
  "4": "flex flex-col h-full w-full bg-surface p-space-1",
  "8": "flex flex-col h-full w-full bg-surface p-space-2",
  "16": "flex flex-col h-full w-full bg-surface p-space-4",
} as const;

const WidgetContext = createContext<UiText | undefined>(undefined);
export const useWidgetTitle = (): UiText | undefined => use(WidgetContext);

type Props = {
  readonly panel: DockPanel;
  readonly title: UiText;
  readonly children: ReactElement;
};
const FrameBody = (props: {
  readonly panel: DockPanel;
  readonly inset: "0" | "4" | "8" | "16";
  readonly children: ReactElement;
}): ReactElement => (
  <div
    data-widget-frame={props.panel}
    className={frameClasses[props.inset]}
    id={props.panel === "briefing" ? "daily-briefing" : props.panel}
    tabIndex={-1}
  >
    {props.children}
  </div>
);
export const WidgetFrame = (props: Props): ReactElement => {
  const appearance = useWidgetAppearance();
  return (
    <WidgetContext value={props.title}>
      <FrameBody panel={props.panel} inset={appearance.padding}>
        {props.children}
      </FrameBody>
    </WidgetContext>
  );
};
