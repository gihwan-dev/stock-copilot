import type { ReactElement } from "react";
import {
  chartCoordinate as c,
  chartPath,
  type ChartCoordinate,
  type ChartDecimal,
} from "../lib";
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
const coordinateSupport = (point: ChartPoint): "valid" | "invalid" => {
  if (!/^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?$/u.test(point.value))
    return "invalid";
  const decimalPoint = point.value.indexOf(".");
  const lastNonzero = Array.from(point.value).findLastIndex(
    (digit) => digit !== "0",
  );
  const coordinateEnd =
    lastNonzero === decimalPoint ? decimalPoint : lastNonzero + 1;
  const normalized =
    decimalPoint < 0 ? point.value : point.value.slice(0, coordinateEnd);
  if (!/^-?(?:0|[1-9][0-9]*)(?:\.[0-9]{1,15})?$/u.test(normalized))
    return "invalid";
  return normalized.replace(/[-.]/gu, "").replace(/^0+/u, "").length > 15
    ? "invalid"
    : "valid";
};
export const chartValues = (data: readonly ChartPoint[]): Values => {
  if (data.length === 0) return { kind: "empty" };
  if (
    data.length > 500 ||
    new Set(data.map((p) => p.id)).size !== data.length ||
    data.some(
      (p) => p.label.trim() === "" || coordinateSupport(p) === "invalid",
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
      readonly minimumValue: ChartDecimal;
      readonly maximumValue: ChartDecimal;
      readonly maximum: ChartCoordinate;
      readonly points: readonly PlotPoint[];
    }
  | { readonly kind: "empty" | "invalid" };
export const lineCoordinates = (data: readonly ChartPoint[]): LineGeometry => {
  const values = chartValues(data);
  if (values.kind !== "ready") return values;
  const minimum = c(Math.min(...values.points.map((p) => p.coordinate)));
  const maximum = c(Math.max(...values.points.map((p) => p.coordinate)));
  const minimumPoint = values.points.find(
    (point) => point.coordinate === minimum,
  );
  const maximumPoint = values.points.find(
    (point) => point.coordinate === maximum,
  );
  if (minimumPoint === undefined || maximumPoint === undefined)
    return { kind: "invalid" };

  const points = values.points.map((p, index) => ({
    id: p.id,
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
  return {
    kind: "ready",
    minimum,
    maximum,
    minimumValue: minimumPoint.value,
    maximumValue: maximumPoint.value,
    points,
  };
};
export const lineGeometry = (
  props: ChartProps,
  mode: "line" | "area",
): ChartGeometry => {
  const geometry = lineCoordinates(props.data);
  if (geometry.kind !== "ready") return geometry;
  const first = geometry.points.at(0);
  const last = geometry.points.at(-1);
  if (first === undefined || last === undefined) return { kind: "invalid" };
  const axes = {
    minimum: geometry.minimumValue,
    maximum: geometry.maximumValue,
    first: first.label,
    last: last.label,
  };
  const path = chartPath(
    geometry.points
      .map(
        (point, index) =>
          `${index === 0 ? "M" : "L"}${point.x.toString()} ${point.y.toString()}`,
      )
      .join(" "),
  );
  const marks = geometry.points.map((point) => ({
    ...point,
    order: "first" as const,
    path: chartPath(
      `M${point.x.toString()} ${point.y.toString()} m-1 0 a1 1 0 1 0 2 0 a1 1 0 1 0 -2 0`,
    ),
  }));
  if (mode === "line")
    return { kind: "ready", layout: "trend", marks, path, axes };
  return {
    kind: "ready",
    layout: "trend",
    marks,
    path,
    axes,
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
