import { createContext, use, type ReactElement } from "react";
import { text, type UiText } from "../lib";
import { ScrollArea } from "./ScrollArea";
import { useWidgetTitle } from "./WidgetFrame";
import { useScrollOwner } from "./scroll-owner";
type Children = { readonly children?: ReactElement | readonly ReactElement[] };
type Density = "normal" | "dense";
const DensityContext = createContext<Density>("normal");
type Frame = "standalone" | "embedded" | "positions" | "currency";
const FrameContext = createContext<Frame>("standalone");
type RootProps = Children & {
  readonly caption?: UiText | undefined;
  readonly empty?: UiText;
  readonly dense?: Density;
  readonly frame?: Frame;
};
const TableElement = (props: {
  readonly children: Children["children"];
  readonly caption?: UiText | undefined;
  readonly empty?: UiText | undefined;
}): ReactElement => (
  <table className="w-full h-full border-collapse tabular-nums">
    {props.caption === undefined ? null : (
      <caption className="text-left p-space-3 text-body-sm text-ink-muted">
        {props.caption}
      </caption>
    )}
    {props.children ?? (
      <tbody>
        <tr>
          <td className="py-space-8 px-space-3 text-center text-ink-muted">
            {props.empty ?? text("ds.tableEmpty", "ko")}
          </td>
        </tr>
      </tbody>
    )}
  </table>
);
const surfaceClasses = {
  standalone:
    "bg-surface border-hairline border-solid border-line rounded-lg max-w-full scroll-flex",
  embedded: "bg-surface max-w-full scroll-flex",
  widget: "flex flex-col flex-1 bg-surface w-full max-w-full",
} as const;
const TableSurface = (props: {
  readonly mode: "embedded" | "standalone" | "widget";
  readonly children: ReactElement;
}): ReactElement => (
  <div data-widget-table="" className={surfaceClasses[props.mode]}>
    {props.children}
  </div>
);
const Root = (props: RootProps): ReactElement => {
  const widget = useWidgetTitle();
  const frame = props.frame ?? "standalone";
  const surface = frame === "standalone" ? "standalone" : "embedded";
  return (
    <DensityContext value={props.dense ?? "normal"}>
      <FrameContext value={frame}>
        <TableSurface mode={widget === undefined ? surface : "widget"}>
          {useScrollOwner() === "card" ? (
            <TableElement caption={props.caption} empty={props.empty}>
              {props.children}
            </TableElement>
          ) : (
            <ScrollArea direction="horizontal">
              <TableElement caption={props.caption} empty={props.empty}>
                {props.children}
              </TableElement>
            </ScrollArea>
          )}
        </TableSurface>
      </FrameContext>
    </DensityContext>
  );
};
const Head = (props: Children): ReactElement => (
  <thead
    className={
      useScrollOwner() === "card"
        ? "h-control-height-sm sticky top-border-hairline z-auto bg-surface-sunken outline-focus outline-surface-sunken"
        : "h-control-height-sm"
    }
  >
    {props.children}
  </thead>
);
const body = (
  props: Children & { readonly group?: UiText | undefined },
): ReactElement => (
  <tbody>
    {props.group === undefined ? null : (
      <tr className="h-control-height-sm">
        <th
          scope="rowgroup"
          colSpan={4}
          className="text-left px-space-3 py-space-2 border-t-hairline border-b-hairline border-line-strong text-label text-ink-muted bg-surface-sunken"
        >
          {props.group}
        </th>
      </tr>
    )}
    {props.children}
  </tbody>
);
const row = (props: Children): ReactElement => (
  <tr className="hover:bg-surface-sunken">{props.children}</tr>
);
type CellProps = {
  readonly children?: UiText | ReactElement;
  readonly align?: "start" | "end";
};
const headerStyles = {
  start:
    "text-left px-space-3 py-space-2 bg-surface-sunken text-ink-muted text-label",
  end: "text-right px-space-3 py-space-2 bg-surface-sunken text-ink-muted text-label",
} as const;
const embeddedHeaderStyles = {
  start: "text-left px-space-3 py-space-2 text-ink-muted text-label",
  end: "whitespace-nowrap text-right px-space-3 py-space-2 text-ink-muted text-label",
} as const;
const stickyHeaderStyles = {
  start: "text-left bg-surface-sunken text-ink-muted text-label",
  end: "text-right bg-surface-sunken text-ink-muted text-label",
  name: "w-full text-left bg-surface-sunken text-ink-muted text-label",
} as const;
const StickyHeader = (props: {
  readonly alignment: "start" | "end" | "name";
  readonly children: CellProps["children"];
}): ReactElement => (
  <th scope="col" className={stickyHeaderStyles[props.alignment]}>
    <span className="block px-space-3 py-space-2 border-b-hairline border-solid border-line">
      {props.children}
    </span>
  </th>
);
const normalCells = {
  start:
    "h-row-height px-space-3 border-t-hairline border-line text-body text-left",
  end: "h-row-height px-space-3 border-t-hairline border-line text-body text-right",
} as const;
const denseCells = {
  start:
    "h-row-height-dense px-space-3 border-t-hairline border-line text-body-sm text-left",
  end: "h-row-height-dense px-space-3 border-t-hairline border-line text-body-sm text-right",
} as const;
const normalHeaders = {
  start:
    "h-row-height px-space-3 font-regular border-t-hairline border-line text-body text-left",
  end: "h-row-height px-space-3 font-regular border-t-hairline border-line text-body text-right",
} as const;
const denseHeaders = {
  start:
    "h-row-height-dense px-space-3 font-regular border-t-hairline border-line text-body-sm text-left",
  end: "h-row-height-dense px-space-3 font-regular border-t-hairline border-line text-body-sm text-right",
} as const;
const ColumnHeader = (props: CellProps): ReactElement => {
  const owner = useScrollOwner();
  const frame = use(FrameContext);
  const align = props.align ?? "start";
  if (owner === "card")
    return (
      <StickyHeader
        alignment={frame === "positions" && align === "start" ? "name" : align}
      >
        {props.children}
      </StickyHeader>
    );
  if (frame === "positions" && align === "start")
    return (
      <th
        scope="col"
        className="w-full text-left px-space-3 py-space-2 text-ink-muted text-label"
      >
        {props.children}
      </th>
    );
  return (
    <th
      scope="col"
      className={
        frame === "standalone"
          ? headerStyles[align]
          : embeddedHeaderStyles[align]
      }
    >
      {props.children}
    </th>
  );
};
const Cell = (props: CellProps): ReactElement => {
  const dense = use(DensityContext);
  const align = props.align ?? "start";
  if (use(FrameContext) === "positions")
    return (
      <td className="px-space-3 py-space-1 border-t-hairline border-line text-right text-body-sm text-ink-muted">
        <div className="h-full flex flex-col justify-center items-end">
          {props.children}
        </div>
      </td>
    );
  return (
    <td className={dense === "dense" ? denseCells[align] : normalCells[align]}>
      {props.children}
    </td>
  );
};
const RowHeader = (props: CellProps): ReactElement => {
  const dense = use(DensityContext);
  const align = props.align ?? "start";
  const frame = use(FrameContext);
  if (frame === "currency")
    return (
      <th
        scope="row"
        className="whitespace-nowrap h-row-height-dense px-space-3 font-regular border-t-hairline border-line text-body-sm text-left"
      >
        {props.children}
      </th>
    );
  if (frame === "positions")
    return (
      <th
        scope="row"
        className="px-space-3 py-space-1 border-t-hairline border-line text-left font-regular"
      >
        <div className="h-full flex flex-col justify-center items-start">
          {props.children}
        </div>
      </th>
    );
  return (
    <th
      scope="row"
      className={dense === "dense" ? denseHeaders[align] : normalHeaders[align]}
    >
      {props.children}
    </th>
  );
};
export const Table = {
  Root,
  Head,
  Body: body,
  Row: row,
  Header: ColumnHeader,
  RowHeader,
  Cell,
} as const;
