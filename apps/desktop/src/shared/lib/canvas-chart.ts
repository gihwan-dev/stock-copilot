import type { CanvasColor, ChartCoordinate, EntityId } from "./brand";

export type CandleThemePalette = {
  readonly surface: CanvasColor;
  readonly grid: CanvasColor;
  readonly axisText: CanvasColor;
  readonly rising: CanvasColor;
  readonly falling: CanvasColor;
  readonly flat: CanvasColor;
  readonly focus: CanvasColor;
};

export type CanvasCandleMark = {
  readonly id: EntityId;
  readonly x: ChartCoordinate;
  readonly span: ChartCoordinate;
  readonly wickTop: ChartCoordinate;
  readonly wickBottom: ChartCoordinate;
  readonly bodyTop: ChartCoordinate;
  readonly bodyBottom: ChartCoordinate;
  readonly direction: "up" | "down" | "flat";
};

export type CandleCanvasScene = {
  readonly theme: "light" | "dark";
  readonly bars: readonly CanvasCandleMark[];
  readonly activeId: EntityId | undefined;
  readonly palette: CandleThemePalette | undefined;
};
