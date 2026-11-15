import type { ChartDecimal, ChartCoordinate, ChartPath, UiText } from "../lib";
export type ChartPoint = {
  readonly label: UiText;
  readonly value: ChartDecimal;
};
export type ChartProps = {
  readonly data: readonly ChartPoint[];
  readonly label: UiText;
  readonly empty: UiText;
  readonly theme: "light" | "dark";
};
export type NumericPoint = ChartPoint & {
  readonly coordinate: ChartCoordinate;
};
export type PlotPoint = ChartPoint & {
  readonly x: ChartCoordinate;
  readonly y: ChartCoordinate;
};
export type ChartMark = ChartPoint & {
  readonly path: ChartPath;
  readonly order: "first" | "second" | "third" | "fourth" | "fifth" | "sixth";
};
export type ChartGeometry =
  | {
      readonly kind: "ready";
      readonly marks: readonly ChartMark[];
      readonly path?: ChartPath;
      readonly area?: ChartPath;
    }
  | { readonly kind: "empty" | "invalid" };
