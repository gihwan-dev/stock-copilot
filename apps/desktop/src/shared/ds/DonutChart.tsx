import type { ReactElement } from "react";
import {
  chartCoordinate as c,
  chartPath,
  type ChartCoordinate,
  type ChartPath,
} from "../lib";
import type {
  ChartPoint,
  ChartProps,
  NumericPoint,
  ChartMark,
} from "./chart-types";
import { chartValues } from "./LineChart";
import { ChartFrame } from "./ChartFrame";
type Arc = NumericPoint &
  ChartMark & {
    readonly start: ChartCoordinate;
    readonly end: ChartCoordinate;
    readonly path: ChartPath;
  };
type Arcs =
  | { readonly kind: "ready"; readonly arcs: readonly Arc[] }
  | { readonly kind: "empty" | "invalid" };
const arcPath = (start: ChartCoordinate, end: ChartCoordinate): ChartPath => {
  if (end - start === 2 * Math.PI)
    return chartPath(
      "M50 5 A25 25 0 1 1 50 55 A25 25 0 1 1 50 5 L50 15 A15 15 0 1 0 50 45 A15 15 0 1 0 50 15 Z",
    );
  const a = start - Math.PI / 2;
  const b = end - Math.PI / 2;
  const large = end - start > Math.PI ? 1 : 0;
  return chartPath(
    `M${(50 + 25 * Math.cos(a)).toString()} ${(30 + 25 * Math.sin(a)).toString()} A25 25 0 ${large.toString()} 1 ${(50 + 25 * Math.cos(b)).toString()} ${(30 + 25 * Math.sin(b)).toString()} L${(50 + 15 * Math.cos(b)).toString()} ${(30 + 15 * Math.sin(b)).toString()} A15 15 0 ${large.toString()} 0 ${(50 + 15 * Math.cos(a)).toString()} ${(30 + 15 * Math.sin(a)).toString()} Z`,
  );
};
export const donutArcs = (data: readonly ChartPoint[]): Arcs => {
  const values = chartValues(data);
  if (values.kind !== "ready") return values;
  if (data.length > 6 || values.points.some((p) => p.coordinate < 0))
    return { kind: "invalid" };
  const points = values.points.filter((p) => p.coordinate > 0);
  const total = points.reduce((sum, p) => c(sum + p.coordinate), c(0));
  if (total === 0) return { kind: "empty" };
  if (points.some((p) => p.coordinate / total < 0.005))
    return { kind: "invalid" };
  const arcs = points.map((p, i) => {
    const before = points.slice(0, i).reduce((sum, v) => sum + v.coordinate, 0);
    const start = c((before / total) * 2 * Math.PI);
    const end = c(
      i === points.length - 1
        ? 2 * Math.PI
        : ((before + p.coordinate) / total) * 2 * Math.PI,
    );
    return {
      ...p,
      order:
        (["first", "second", "third", "fourth", "fifth", "sixth"] as const).at(
          i,
        ) ?? "first",
      start,
      end,
      path: arcPath(start, end),
    };
  });
  return { kind: "ready", arcs };
};
export const DonutChart = (props: ChartProps): ReactElement => {
  const geometry = donutArcs(props.data);
  return (
    <ChartFrame
      data={props.data}
      label={props.label}
      empty={props.empty}
      theme={props.theme}
      geometry={
        geometry.kind === "ready"
          ? { kind: "ready", layout: "donut", marks: geometry.arcs }
          : geometry
      }
    />
  );
};
