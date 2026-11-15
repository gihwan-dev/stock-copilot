import type { ReactElement } from "react";
import type { ChartProps } from "./chart-types";
import { lineGeometry } from "./LineChart";
import { ChartFrame } from "./ChartFrame";
export const AreaChart = (props: ChartProps): ReactElement => (
  <ChartFrame
    data={props.data}
    label={props.label}
    empty={props.empty}
    theme={props.theme}
    geometry={lineGeometry(props, "area")}
  />
);
