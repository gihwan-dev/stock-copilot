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
type Rectangle = NumericPoint &
  ChartMark & {
    readonly x: ChartCoordinate;
    readonly width: ChartCoordinate;
    readonly height: ChartCoordinate;
    readonly path: ChartPath;
  };
type Rectangles =
  | { readonly kind: "ready"; readonly rectangles: readonly Rectangle[] }
  | { readonly kind: "empty" | "invalid" };
export const treemapRectangles = (data: readonly ChartPoint[]): Rectangles => {
  const values = chartValues(data);
  if (values.kind !== "ready") return values;
  if (data.length > 6 || values.points.some((p) => p.coordinate < 0))
    return { kind: "invalid" };
  const points = values.points.filter((p) => p.coordinate > 0);
  const total = points.reduce((sum, p) => c(sum + p.coordinate), c(0));
  if (total === 0) return { kind: "empty" };
  if (points.some((p) => p.coordinate / total < 0.005))
    return { kind: "invalid" };
  const rectangles = points.map((p, i) => {
    const before = points.slice(0, i).reduce((sum, v) => sum + v.coordinate, 0);
    const x = c((100 * before) / total);
    const end = c(
      i === points.length - 1 ? 100 : (100 * (before + p.coordinate)) / total,
    );
    return {
      ...p,
      order:
        (["first", "second", "third", "fourth", "fifth", "sixth"] as const).at(
          i,
        ) ?? "first",
      x,
      width: c(end - x),
      height: c(60),
      path: chartPath(
        `M${x.toString()} 0 H${end.toString()} V60 H${x.toString()} Z`,
      ),
    };
  });
  return { kind: "ready", rectangles };
};
export const Treemap = (props: ChartProps): ReactElement => {
  const geometry = treemapRectangles(props.data);
  return (
    <ChartFrame
      data={props.data}
      label={props.label}
      empty={props.empty}
      theme={props.theme}
      geometry={
        geometry.kind === "ready"
          ? { kind: "ready", layout: "treemap", marks: geometry.rectangles }
          : geometry
      }
    />
  );
};
