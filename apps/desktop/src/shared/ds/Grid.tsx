import type { ReactElement } from "react";
import type { LayoutChildren } from "./layout-children";
import { ScrollArea } from "./ScrollArea";

type Layout = "flow" | "fit-row" | "fill-row";

type Props = LayoutChildren & {
  readonly layout?: Layout;
};

const rowClasses = {
  "fit-row": "@container flex flex-col flex-none w-full max-w-full scroll-flex",
  "fill-row": "@container flex flex-col flex-1 w-full max-w-full scroll-flex",
} as const;

const Row = (
  props: LayoutChildren & { readonly layout: "fit-row" | "fill-row" },
): ReactElement => (
  <div className={rowClasses[props.layout]}>
    <div className="grid grid-cols-layout gap-space-4 flex-1 w-full max-w-full scroll-flex">
      {props.children}
    </div>
  </div>
);

export const Grid = (props: Props): ReactElement => {
  const layout = props.layout ?? "flow";
  if (layout === "flow")
    return (
      <div className="@container w-full max-w-full scroll-flex">
        <ScrollArea direction="horizontal">
          <div className="grid grid-cols-layout gap-space-6 w-full max-w-full scroll-flex">
            {props.children}
          </div>
        </ScrollArea>
      </div>
    );
  return <Row layout={layout}>{props.children}</Row>;
};
