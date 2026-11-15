import { createContext, use, type ReactElement } from "react";
import { text, type UiText } from "../lib";
type Children = { readonly children?: ReactElement | readonly ReactElement[] };
type Density = "normal" | "dense";
const DensityContext = createContext<Density>("normal");
const root = (
  props: Children & {
    readonly caption?: UiText;
    readonly empty?: UiText;
    readonly dense?: Density;
  },
): ReactElement => (
  <DensityContext value={props.dense ?? "normal"}>
    <div className="bg-surface border-hairline border-solid border-line rounded-lg">
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
    </div>
  </DensityContext>
);
const head = (props: Children): ReactElement => <thead>{props.children}</thead>;
const body = (props: Children): ReactElement => <tbody>{props.children}</tbody>;
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
const header = (props: CellProps): ReactElement => {
  const align = props.align ?? "start";
  return (
    <th scope="col" className={headerStyles[align]}>
      {props.children}
    </th>
  );
};
const Cell = (props: CellProps): ReactElement => {
  const dense = use(DensityContext);
  const align = props.align ?? "start";
  return (
    <td className={dense === "dense" ? denseCells[align] : normalCells[align]}>
      {props.children}
    </td>
  );
};
const RowHeader = (props: CellProps): ReactElement => {
  const dense = use(DensityContext);
  const align = props.align ?? "start";
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
  Root: root,
  Head: head,
  Body: body,
  Row: row,
  Header: header,
  RowHeader,
  Cell,
} as const;
