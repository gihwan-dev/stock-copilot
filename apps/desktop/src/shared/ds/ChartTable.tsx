import type { ReactElement } from "react";
import type { ChartProps } from "./chart-types";
export const ChartTable = (props: ChartProps): ReactElement => (
  <table className="w-full text-body-sm tabular-nums">
    <caption>{props.label}</caption>
    <tbody>
      {props.data.map((point) => (
        <tr key={point.label}>
          <th scope="row" className="text-left status-sentence">
            {point.label}
          </th>
          <td className="text-right status-sentence">{point.value}</td>
        </tr>
      ))}
    </tbody>
  </table>
);
