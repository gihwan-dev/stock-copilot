import type { ReactElement } from "react";
import type { LayoutChildren } from "./layout-children";

export const Toolbar = (props: LayoutChildren): ReactElement => (
  <div className="flex flex-wrap items-center gap-space-2 min-h-toolbar-height py-space-2 px-space-4 w-full max-w-full scroll-flex">
    {props.children}
  </div>
);
