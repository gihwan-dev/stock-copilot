import type { ReactElement } from "react";
import { useWidgetTitle } from "./WidgetFrame";
import type { SizeToken } from "./tokens";

type SplitWidthToken = Extract<
  SizeToken,
  "sidebar-width" | "sidebar-collapsed" | "chat-width" | "chat-width-min"
>;

type Props = {
  readonly size: SplitWidthToken;
  readonly secondaryFooter?: ReactElement;
  readonly children: readonly [ReactElement, ReactElement];
};

const fixedPaneClasses = {
  "sidebar-width": "flex-none w-sidebar-width max-w-full scroll-flex",
  "sidebar-collapsed": "flex-none w-sidebar-collapsed max-w-full scroll-flex",
  "chat-width": "flex-none w-chat-width max-w-full scroll-flex",
  "chat-width-min": "flex-none w-chat-width-min max-w-full scroll-flex",
} as const;

type WidthKey =
  "sidebar-width" | "sidebar-collapsed" | "chat-width" | "chat-width-min";
const SplitBox = (props: {
  readonly pane: WidthKey;
  readonly secondaryFooter?: ReactElement | undefined;
  readonly children: readonly [ReactElement, ReactElement];
}): ReactElement => (
  <div
    data-split-content=""
    className="flex flex-wrap gap-space-6 w-full max-w-full scroll-flex"
  >
    <div className={fixedPaneClasses[props.pane]}>{props.children[0]}</div>
    <div className="flex-1 wrap-slot max-w-full scroll-flex">
      <div className="flex flex-col gap-space-2 w-full max-w-full scroll-flex">
        {props.children[1]}
        {props.secondaryFooter}
      </div>
    </div>
  </div>
);

export const Split = (props: Props): ReactElement =>
  useWidgetTitle() === undefined ? (
    <SplitBox pane={props.size} secondaryFooter={props.secondaryFooter}>
      {props.children}
    </SplitBox>
  ) : (
    <div
      data-split-content=""
      className="flex flex-1 flex-wrap gap-space-2 w-full max-w-full"
    >
      <div className="flex flex-1 flex-col gap-space-2 max-w-full">
        {props.children[0]}
      </div>
      <div className="flex flex-1 flex-col gap-space-2 max-w-full">
        {props.children[1]}
        {props.secondaryFooter === undefined ? null : (
          <div className="flex flex-1 flex-col gap-space-1 justify-between">
            {props.secondaryFooter}
          </div>
        )}
      </div>
    </div>
  );
