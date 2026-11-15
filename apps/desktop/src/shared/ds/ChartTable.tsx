import type { ReactElement } from "react";
import type { ChartProps, ChartPoint } from "./chart-types";
type Props = ChartProps & {
  readonly onHighlight: (point: ChartPoint | undefined) => void;
  readonly layout: "trend" | "donut" | "treemap";
};
export const ChartTable = (props: Props): ReactElement => (
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
          <td className="text-right status-sentence">{point.value}</td>
        </tr>
      ))}
    </tbody>
  </table>
);
const classes = {
  trend: "sr-only",
  donut: "w-full text-body-sm tabular-nums",
  treemap: "w-full text-body-sm tabular-nums",
} as const;
