import type { ReactElement } from "react";
import type {
  ChartProps,
  ChartPoint,
  ChartAxisLabels,
  ChartMark,
} from "./chart-types";
import { ChartNumber } from "./Numbers";
import { ScrollArea } from "./ScrollArea";
import { text } from "../lib";
type Props = ChartProps & {
  readonly onHighlight: (point: ChartPoint | undefined) => void;
  readonly layout: "trend" | "donut" | "treemap";
};
const ChartTableContent = (props: Props): ReactElement => (
  <table className={classes[props.layout]}>
    <caption>{props.label}</caption>
    <tbody>
      {props.data.map((point) => (
        <tr
          key={point.id}
          onPointerEnter={(): void => {
            props.onHighlight(point);
          }}
          onPointerLeave={(): void => {
            props.onHighlight(undefined);
          }}
        >
          <th scope="row" className="text-left status-sentence">
            {point.label}
          </th>
          <td className="text-right status-sentence">
            <ChartNumber value={point.value} />
          </td>
        </tr>
      ))}
    </tbody>
  </table>
);
export const ChartTable = (props: Props): ReactElement => {
  const content = (
    <ChartTableContent
      data={props.data}
      label={props.label}
      empty={props.empty}
      theme={props.theme}
      onHighlight={props.onHighlight}
      layout={props.layout}
    />
  );
  return props.layout === "trend" ? (
    <div className="sr-only">{content}</div>
  ) : (
    <ScrollArea direction="horizontal">{content}</ScrollArea>
  );
};
const classes = {
  trend: "sr-only",
  donut: "w-full text-body-sm tabular-nums",
  treemap: "w-full text-body-sm tabular-nums",
} as const;

export const ChartAxes = (props: {
  readonly axes: ChartAxisLabels;
}): ReactElement => (
  <div className="flex flex-col gap-space-1">
    <div className="flex flex-col @content-medium:flex-row justify-between gap-space-2">
      <span className="text-caption text-ink-muted status-sentence">
        {props.axes.first}
      </span>
      <span className="text-caption text-ink-muted status-sentence">
        {props.axes.last}
      </span>
    </div>
    <div className="flex flex-col @content-medium:flex-row justify-between gap-space-2">
      <ScrollArea direction="horizontal">
        <span className="block w-max text-caption text-ink-muted">
          {text("ds.chartMinimum", "ko")}
          {text("ds.chartSeparator", "ko")}
          <ChartNumber value={props.axes.minimum} />
        </span>
      </ScrollArea>
      <ScrollArea direction="horizontal">
        <div className="w-full @content-medium:flex @content-medium:justify-end">
          <span className="block w-max text-caption text-ink-muted">
            {text("ds.chartMaximum", "ko")}
            {text("ds.chartSeparator", "ko")}
            <ChartNumber value={props.axes.maximum} />
          </span>
        </div>
      </ScrollArea>
    </div>
  </div>
);

export const ChartStatusBody = (props: {
  readonly selected: ChartMark | undefined;
  readonly keepMounted: "keep" | "hide";
}): ReactElement => (
  <>
    {props.selected === undefined ? null : (
      <span className="status-sentence">
        {props.selected.label}
        {text("ds.chartSeparator", "ko")}
      </span>
    )}
    {props.selected === undefined && props.keepMounted === "hide" ? null : (
      <ScrollArea direction="horizontal">
        {props.selected === undefined ? (
          <span />
        ) : (
          <ChartNumber value={props.selected.value} />
        )}
      </ScrollArea>
    )}
  </>
);
