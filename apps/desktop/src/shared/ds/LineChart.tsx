import type { ReactElement } from "react";
import { chartCoordinate as c, chartPath, type ChartCoordinate } from "../lib";
import type {
  ChartProps,
  ChartPoint,
  NumericPoint,
  PlotPoint,
  ChartGeometry,
} from "./chart-types";
import { ChartFrame } from "./ChartFrame";
type Values =
  | { readonly kind: "ready"; readonly points: readonly NumericPoint[] }
  | { readonly kind: "empty" | "invalid" };
export const chartValues = (data: readonly ChartPoint[]): Values => {
  if (data.length === 0) return { kind: "empty" };
  if (
    data.length > 500 ||
    new Set(data.map((p) => p.label)).size !== data.length ||
    data.some(
      (p) =>
        p.label.trim() === "" ||
        !/^-?(?:0|[1-9][0-9]*)(?:\.[0-9]{1,15})?$/u.test(p.value) ||
        p.value.replace(/[-.]/gu, "").replace(/^0+/u, "").length > 15,
    )
  )
    return { kind: "invalid" };
  const points = data.map((p) => ({ ...p, coordinate: c(Number(p.value)) }));
  return points.some((p) => !Number.isFinite(p.coordinate))
    ? { kind: "invalid" }
    : { kind: "ready", points };
};
type LineGeometry =
  | {
      readonly kind: "ready";
      readonly minimum: ChartCoordinate;
      readonly maximum: ChartCoordinate;
      readonly points: readonly PlotPoint[];
    }
  | { readonly kind: "empty" | "invalid" };
export const lineCoordinates = (data: readonly ChartPoint[]): LineGeometry => {
  const values = chartValues(data);
  if (values.kind !== "ready") return values;
  const minimum = c(Math.min(...values.points.map((p) => p.coordinate)));
  const maximum = c(Math.max(...values.points.map((p) => p.coordinate)));
  const points = values.points.map((p, index) => ({
    label: p.label,
    value: p.value,
    x: c(
      values.points.length === 1
        ? 50
        : 5 + (90 * index) / (values.points.length - 1),
    ),
    y: c(
      maximum === minimum
        ? 30
        : 55 - (50 * (p.coordinate - minimum)) / (maximum - minimum),
    ),
  }));
  return { kind: "ready", minimum, maximum, points };
};
export const lineGeometry = (
  props: ChartProps,
  mode: "line" | "area",
): ChartGeometry => {
  const geometry = lineCoordinates(props.data);
  if (geometry.kind !== "ready") return geometry;
  const path = chartPath(
    geometry.points
      .map(
        (p, i) => `${i === 0 ? "M" : "L"}${p.x.toString()} ${p.y.toString()}`,
      )
      .join(" "),
  );
  const marks = geometry.points.map((p) => ({
    ...p,
    order: "first" as const,
    path: chartPath(
      `M${p.x.toString()} ${p.y.toString()} m-1 0 a1 1 0 1 0 2 0 a1 1 0 1 0 -2 0`,
    ),
  }));
  if (mode === "line") return { kind: "ready", marks, path };
  const first = geometry.points.at(0);
  const last = geometry.points.at(-1);
  if (first === undefined || last === undefined) return { kind: "invalid" };
  return {
    kind: "ready",
    marks,
    path,
    area: chartPath(
      `${path} L${last.x.toString()} 55 L${first.x.toString()} 55 Z`,
    ),
  };
};
export const LineChart = (props: ChartProps): ReactElement => (
  <ChartFrame
    data={props.data}
    label={props.label}
    empty={props.empty}
    theme={props.theme}
    geometry={lineGeometry(props, "line")}
  />
);
