import type { ReactElement } from "react";
import { ScrollArea as BaseScrollArea } from "@base-ui/react/scroll-area";

type Direction = "horizontal" | "vertical" | "both";
type Props = {
  readonly direction: Direction;
  readonly children:
    ReactElement | readonly (ReactElement | readonly ReactElement[])[];
};

const rootClasses = {
  horizontal: "w-full max-w-full scroll-flex",
  vertical: "flex-1 scroll-flex flex flex-col w-full max-w-full",
  both: "flex-1 scroll-flex flex flex-col w-full max-w-full",
} as const;

const viewportClasses = {
  horizontal:
    "scroll-viewport-x w-full max-w-full scroll-flex pt-gap-hairline px-gap-hairline pb-space-2 focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:scroll-outline-inset",
  vertical:
    "scroll-viewport-y flex-1 scroll-flex w-full max-w-full py-gap-hairline focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:scroll-outline-inset",
  both: "scroll-viewport-both flex-1 scroll-flex w-full max-w-full p-gap-hairline focus-visible:outline-solid focus-visible:outline-focus focus-visible:outline-focus-ring focus-visible:scroll-outline-inset",
} as const;

const contentClasses = {
  horizontal: "w-full",
  vertical: "w-full scroll-content-y py-gap-hairline",
  both: "w-full",
} as const;

const Scrollbars = (props: { readonly direction: Direction }): ReactElement => {
  if (props.direction === "horizontal")
    return (
      <BaseScrollArea.Scrollbar
        orientation="horizontal"
        className="flex p-gap-hairline scroll-track-x overflow-hidden select-none"
      >
        <BaseScrollArea.Thumb className="h-full rounded-pill bg-line-strong" />
      </BaseScrollArea.Scrollbar>
    );
  if (props.direction === "vertical")
    return (
      <BaseScrollArea.Scrollbar
        orientation="vertical"
        className="flex p-gap-hairline scroll-track-y overflow-hidden select-none"
      >
        <BaseScrollArea.Thumb className="w-full rounded-pill bg-line-strong" />
      </BaseScrollArea.Scrollbar>
    );
  return (
    <>
      <BaseScrollArea.Scrollbar
        orientation="horizontal"
        className="flex p-gap-hairline scroll-track-x overflow-hidden select-none"
      >
        <BaseScrollArea.Thumb className="h-full rounded-pill bg-line-strong" />
      </BaseScrollArea.Scrollbar>
      <BaseScrollArea.Scrollbar
        orientation="vertical"
        className="flex p-gap-hairline scroll-track-y overflow-hidden select-none"
      >
        <BaseScrollArea.Thumb className="w-full rounded-pill bg-line-strong" />
      </BaseScrollArea.Scrollbar>
      <BaseScrollArea.Corner />
    </>
  );
};

export const ScrollArea = (props: Props): ReactElement => (
  <BaseScrollArea.Root className={rootClasses[props.direction]}>
    <BaseScrollArea.Viewport
      data-scroll-viewport=""
      className={viewportClasses[props.direction]}
    >
      <BaseScrollArea.Content className={contentClasses[props.direction]}>
        {props.children}
      </BaseScrollArea.Content>
    </BaseScrollArea.Viewport>
    <Scrollbars direction={props.direction} />
  </BaseScrollArea.Root>
);
