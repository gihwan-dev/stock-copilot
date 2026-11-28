import { createContext, use, type ReactElement } from "react";
import { text, type UiText } from "../lib";
import { ScrollArea } from "./ScrollArea";
import { useScrollOwner } from "./scroll-owner";
type Children = { readonly children?: ReactElement | readonly ReactElement[] };
type Density = "normal" | "dense";
const DensityContext = createContext<Density>("normal");
type Frame = "standalone" | "embedded" | "positions";
const FrameContext = createContext<Frame>("standalone");
type RootProps = Children & {
  readonly caption?: UiText | undefined;
  readonly empty?: UiText;
  readonly dense?: Density;
  readonly frame?: Frame;
};
const TableElement = (props: {
  readonly children: ReactElement | readonly ReactElement[] | undefined;
  readonly caption: UiText | undefined;
  readonly empty: UiText | undefined;
}): ReactElement => (
  <table className="w-full border-collapse tabular-nums">
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
const Root = (props: RootProps): ReactElement => (
  <DensityContext value={props.dense ?? "normal"}>
    <FrameContext value={props.frame ?? "standalone"}>
      <div
        className={
          props.frame === "embedded" || props.frame === "positions"
            ? "bg-surface max-w-full scroll-flex"
            : "bg-surface border-hairline border-solid border-line rounded-lg max-w-full scroll-flex"
        }
      >
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
      </div>
    </FrameContext>
  </DensityContext>
);
const head = (props: Children): ReactElement => <thead>{props.children}</thead>;
const body = (
  props: Children & { readonly group?: UiText | undefined },
): ReactElement => (
  <tbody>
    {props.group === undefined ? null : (
      <tr>
        <th
          scope="rowgroup"
          colSpan={4}
          className="text-left px-space-3 pt-space-3 pb-space-1 text-label text-ink-muted bg-surface-sunken"
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
  start:
    "sticky top-border-hairline z-sticky text-left px-space-3 py-space-2 bg-surface-sunken text-ink-muted text-label",
  end: "sticky top-border-hairline z-sticky text-right px-space-3 py-space-2 bg-surface-sunken text-ink-muted text-label",
} as const;
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
  if (frame === "positions" && align === "start")
    return (
      <th
        scope="col"
        className="w-chat-width text-left px-space-3 py-space-2 text-ink-muted text-label"
      >
        {props.children}
      </th>
    );
  if (frame !== "standalone" && owner === "own")
    return (
      <th scope="col" className={embeddedHeaderStyles[align]}>
        {props.children}
      </th>
    );
  return (
    <th
      scope="col"
      className={
        owner === "card" ? stickyHeaderStyles[align] : headerStyles[align]
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
      <td className="h-row-height px-space-3 py-space-1 border-t-hairline border-line-strong text-right text-body-sm text-ink-muted">
        <div className="h-full flex flex-col items-end">{props.children}</div>
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
  if (use(FrameContext) === "positions")
    return (
      <th
        scope="row"
        className="h-row-height px-space-3 py-space-1 border-t-hairline border-line-strong text-left font-regular"
      >
        <div className="h-full flex flex-col items-start">{props.children}</div>
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
  Head: head,
  Body: body,
  Row: row,
  Header: ColumnHeader,
  RowHeader,
  Cell,
} as const;
