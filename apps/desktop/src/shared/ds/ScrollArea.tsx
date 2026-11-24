import type { ReactElement } from "react";
import { ScrollArea as BaseScrollArea } from "@base-ui/react/scroll-area";

type Direction = "horizontal" | "vertical";
type Layout = "plain" | "card" | "dialog" | "stage" | "numeric";
type Props = {
  readonly direction: Direction;
  readonly layout?: Layout;
  readonly children:
    ReactElement | readonly (ReactElement | readonly ReactElement[])[];
};

const rootClasses = {
  plain: "w-full max-w-full scroll-flex",
  card: "w-full max-w-full scroll-flex",
  numeric: "w-full max-w-full scroll-flex",
  dialog: "flex-1 scroll-flex flex flex-col w-full max-w-full",
  stage: "flex-1 scroll-flex flex flex-col w-full max-w-full",
} as const;

const horizontalViewports = {
  plain:
    "scroll-viewport-x w-full max-w-full scroll-flex p-gap-hairline focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:scroll-outline-inset",
  card: "scroll-viewport-x w-full max-w-full scroll-flex p-space-4 flex flex-col gap-space-4 focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:scroll-outline-inset",
  numeric:
    "scroll-viewport-x w-full max-w-full scroll-flex p-gap-hairline focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:scroll-outline-inset",
  dialog:
    "scroll-viewport-x w-full max-w-full scroll-flex p-space-5 flex flex-col gap-space-4 focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:scroll-outline-inset",
  stage:
    "scroll-viewport-x w-full max-w-full scroll-flex py-space-4 focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:scroll-outline-inset",
} as const;

const verticalViewports = {
  plain:
    "scroll-viewport-y flex-1 scroll-flex w-full max-w-full focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:scroll-outline-inset",
  card: "scroll-viewport-y flex-1 scroll-flex w-full max-w-full p-space-4 flex flex-col gap-space-4 focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:scroll-outline-inset",
  numeric:
    "scroll-viewport-y flex-1 scroll-flex w-full max-w-full focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:scroll-outline-inset",
  dialog:
    "scroll-viewport-y flex-1 scroll-flex w-full max-w-full p-space-5 focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:scroll-outline-inset",
  stage:
    "scroll-viewport-y flex-1 scroll-flex w-full max-w-full py-space-4 focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:scroll-outline-inset",
} as const;

export const ScrollArea = (props: Props): ReactElement => {
  const layout = props.layout ?? "plain";
  if (props.direction === "horizontal")
    return (
      <BaseScrollArea.Root className={rootClasses[layout]}>
        <BaseScrollArea.Viewport
          data-scroll-viewport=""
          className={horizontalViewports[layout]}
        >
          {props.children}
        </BaseScrollArea.Viewport>
        <BaseScrollArea.Scrollbar
          orientation="horizontal"
          className="flex p-gap-hairline scroll-track-x overflow-hidden select-none"
        >
          <BaseScrollArea.Thumb className="h-full rounded-pill bg-line-strong" />
        </BaseScrollArea.Scrollbar>
      </BaseScrollArea.Root>
    );
  return (
    <BaseScrollArea.Root className={rootClasses[layout]}>
      <BaseScrollArea.Viewport
        data-scroll-viewport=""
        className={verticalViewports[layout]}
      >
        {props.children}
      </BaseScrollArea.Viewport>
      <BaseScrollArea.Scrollbar
        orientation="vertical"
        className="flex p-gap-hairline scroll-track-y overflow-hidden select-none"
      >
        <BaseScrollArea.Thumb className="w-full rounded-pill bg-line-strong" />
      </BaseScrollArea.Scrollbar>
    </BaseScrollArea.Root>
  );
};
