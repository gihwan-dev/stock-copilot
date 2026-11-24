import type { ReactElement } from "react";
import type { LayoutChildren } from "./layout-children";
import { ScrollArea } from "./ScrollArea";

export const Grid = (props: LayoutChildren): ReactElement => (
  <div className="@container w-full max-w-full scroll-flex">
    <ScrollArea direction="horizontal">
      <div className="grid grid-cols-layout gap-space-6 w-full max-w-full scroll-flex">
        {props.children}
      </div>
    </ScrollArea>
  </div>
);
