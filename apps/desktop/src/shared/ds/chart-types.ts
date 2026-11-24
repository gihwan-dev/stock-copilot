import type {
  CandleThemePalette,
  CanvasCandleMark,
  ChartDecimal,
  ChartCoordinate,
  ChartPath,
  FreeText,
  UiText,
  EntityId,
} from "../lib";
export type ChartPoint = {
  readonly id: EntityId;
  readonly label: UiText | FreeText;
  readonly value: ChartDecimal;
};
export type ChartProps = {
  readonly data: readonly ChartPoint[];
  readonly label: UiText;
  readonly empty: UiText;
  readonly theme: "light" | "dark";
};
export type CandlePoint = {
  readonly id: EntityId;
  readonly label: UiText;
  readonly open: ChartDecimal;
  readonly high: ChartDecimal;
  readonly low: ChartDecimal;
  readonly close: ChartDecimal;
};
export type CandleChartProps = {
  readonly data: readonly CandlePoint[];
  readonly label: UiText;
  readonly empty: UiText;
  readonly theme: "light" | "dark";
  readonly palette?: CandleThemePalette;
};
export type CandleBar = CandlePoint & CanvasCandleMark;
export type NumericCandle = CandlePoint & {
  readonly o: ChartCoordinate;
  readonly h: ChartCoordinate;
  readonly l: ChartCoordinate;
  readonly cl: ChartCoordinate;
};
export type CandleNavProps = {
  readonly data: readonly CandlePoint[];
  readonly label: UiText;
  readonly selected: CandlePoint | undefined;
  readonly onSelect: (id: EntityId | undefined) => void;
};
export type CandleGeometry =
  | {
      readonly kind: "ready";
      readonly minimum: ChartCoordinate;
      readonly maximum: ChartCoordinate;
      readonly axes: ChartAxisLabels;
      readonly bars: readonly CandleBar[];
    }
  | { readonly kind: "empty" | "invalid" };
export type { CandleThemePalette };
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
export type ChartAxisLabels = {
  readonly minimum: ChartDecimal;
  readonly maximum: ChartDecimal;
  readonly first: UiText | FreeText;
  readonly last: UiText | FreeText;
};
export type ChartGeometry =
  | {
      readonly kind: "ready";
      readonly layout: "trend" | "donut" | "treemap";
      readonly axes?: ChartAxisLabels;
      readonly marks: readonly ChartMark[];
      readonly path?: ChartPath;
      readonly area?: ChartPath;
    }
  | { readonly kind: "empty" | "invalid" };
