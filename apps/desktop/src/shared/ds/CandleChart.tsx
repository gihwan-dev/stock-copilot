import { useId, useMemo, useRef, useState, type ReactElement } from "react";
import {
  chartCoordinate as c,
  text,
  type ChartCoordinate,
  type ChartDecimal,
  type EntityId,
} from "../lib";
import { chartNavigation } from "../platform/input";
import { useCandleCanvas } from "../platform/canvasTheme";
import type {
  CandleBar,
  CandleChartProps,
  CandleGeometry,
  CandleNavProps,
  CandlePoint,
  NumericCandle,
} from "./chart-types";
import { EmptyState } from "./EmptyState";
import { CandleStatusBody, CandleTable, ChartAxes } from "./ChartTable";

const decimalOk = (v: ChartDecimal): "ok" | "bad" => {
  if (!/^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?$/u.test(v)) return "bad";
  const dot = v.indexOf(".");
  const last = Array.from(v).findLastIndex((d) => d !== "0");
  const norm = dot < 0 ? v : v.slice(0, last === dot ? dot : last + 1);
  if (!/^-?(?:0|[1-9][0-9]*)(?:\.[0-9]{1,15})?$/u.test(norm)) return "bad";
  return norm.replace(/[-.]/gu, "").replace(/^0+/u, "").length > 15
    ? "bad"
    : "ok";
};
const validCandle = (b: NumericCandle): "ok" | "bad" => {
  if ([b.o, b.h, b.l, b.cl].some((v) => !Number.isFinite(v))) return "bad";
  return b.h >= Math.max(b.o, b.cl) && b.l <= Math.min(b.o, b.cl)
    ? "ok"
    : "bad";
};
const parseCandle = (p: CandlePoint): readonly NumericCandle[] => {
  if (
    p.label.trim() === "" ||
    [p.open, p.high, p.low, p.close].some((v) => decimalOk(v) === "bad")
  )
    return [];
  const candle: NumericCandle = {
    ...p,
    o: c(Number(p.open)),
    h: c(Number(p.high)),
    l: c(Number(p.low)),
    cl: c(Number(p.close)),
  };
  return validCandle(candle) === "ok" ? [candle] : [];
};
const dirOf = (b: NumericCandle): "up" | "down" | "flat" => {
  if (b.cl > b.o) return "up";
  return b.cl < b.o ? "down" : "flat";
};
const py = (
  v: ChartCoordinate,
  min: ChartCoordinate,
  max: ChartCoordinate,
): ChartCoordinate => c(max === min ? 30 : 55 - (50 * (v - min)) / (max - min));
const buildBars = (
  cs: readonly NumericCandle[],
  min: ChartCoordinate,
  max: ChartCoordinate,
): readonly CandleBar[] => {
  const step = 90 / cs.length;
  const span = c(Math.min(12, Math.max(1.2, step * 0.6)));
  return cs.map((b, i) => ({
    ...b,
    x: c(5 + step * (i + 0.5)),
    span,
    wickTop: py(b.h, min, max),
    wickBottom: py(b.l, min, max),
    bodyTop: py(c(Math.max(b.o, b.cl)), min, max),
    bodyBottom: py(c(Math.min(b.o, b.cl)), min, max),
    direction: dirOf(b),
  }));
};
const readyGeometry = (cs: readonly NumericCandle[]): CandleGeometry => {
  const min = c(Math.min(...cs.map((b) => b.l)));
  const max = c(Math.max(...cs.map((b) => b.h)));
  const minC = cs.find((b) => b.l === min);
  const maxC = cs.find((b) => b.h === max);
  const first = cs.at(0);
  const last = cs.at(-1);
  if (
    minC === undefined ||
    maxC === undefined ||
    first === undefined ||
    last === undefined
  )
    return { kind: "invalid" };
  return {
    kind: "ready",
    minimum: min,
    maximum: max,
    axes: {
      minimum: minC.low,
      maximum: maxC.high,
      first: first.label,
      last: last.label,
    },
    bars: buildBars(cs, min, max),
  };
};
export const candleCoordinates = (
  data: readonly CandlePoint[],
): CandleGeometry => {
  if (data.length === 0) return { kind: "empty" };
  if (data.length > 500 || new Set(data.map((p) => p.id)).size !== data.length)
    return { kind: "invalid" };
  const cs = data.flatMap(parseCandle);
  return cs.length === data.length ? readyGeometry(cs) : { kind: "invalid" };
};
const movePoint = (
  pts: readonly CandlePoint[],
  sel: CandlePoint | undefined,
  dir: "next" | "previous" | "first" | "last",
): CandlePoint | undefined => {
  const i = pts.findIndex((p) => p.id === sel?.id);
  if (dir === "first") return pts.at(0);
  if (dir === "last") return pts.at(-1);
  if (dir === "next") return pts.at(i + 1) ?? pts.at(0);
  return pts.at(i <= 0 ? -1 : i - 1);
};
const CandleNav = ({
  data,
  label,
  selected,
  onSelect,
}: CandleNavProps): ReactElement => {
  const id = useId();
  const [saved, setSaved] = useState<EntityId | undefined>(undefined);
  const pick = (pt: CandlePoint | undefined): void => {
    setSaved(pt?.id);
    onSelect(pt?.id);
  };
  return (
    <div className="flex flex-col items-start gap-space-1 w-full scroll-flex">
      <button
        type="button"
        aria-describedby={id}
        className="text-body-sm text-primary rounded-sm px-space-2 py-space-1 focus-visible:outline-focus status-sentence max-w-full text-left"
        onFocus={(): void => {
          pick(selected ?? data.at(0));
        }}
        onBlur={(): void => {
          onSelect(undefined);
        }}
        onKeyDown={(e): void => {
          const d = chartNavigation(e);
          if (d !== "ignore") pick(movePoint(data, selected, d));
        }}
      >
        {text("ds.chartExplore", "ko")}
        {text("ds.chartSeparator", "ko")}
        {label}
      </button>
      <div
        id={id}
        role="status"
        className="text-body-sm flex flex-col gap-space-1 w-full scroll-flex"
        onPointerDown={(): void => {
          if (selected === undefined) setSaved(undefined);
        }}
        onFocus={(): void => {
          if (saved !== undefined) onSelect(saved);
        }}
        onBlur={(): void => {
          pick(undefined);
        }}
      >
        <CandleStatusBody
          selected={selected}
          keepMounted={saved === undefined ? "hide" : "keep"}
        />
      </div>
    </div>
  );
};
const CandleContent = ({
  props,
  geometry,
}: {
  readonly props: CandleChartProps;
  readonly geometry: Extract<CandleGeometry, { readonly kind: "ready" }>;
}): ReactElement => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [focused, setFocused] = useState<EntityId | undefined>(undefined);
  const [hovered, setHovered] = useState<EntityId | undefined>(undefined);
  const active =
    props.data.find((p) => p.id === focused) ??
    props.data.find((p) => p.id === hovered);
  const scene = useMemo(
    () => ({
      theme: props.theme,
      bars: geometry.bars,
      activeId: active?.id,
      palette: props.palette,
    }),
    [props.theme, geometry.bars, active?.id, props.palette],
  );
  useCandleCanvas(canvasRef, scene);
  return (
    <>
      <canvas
        ref={canvasRef}
        role="img"
        aria-label={props.label}
        className="block w-full h-chart-height"
      />
      <ChartAxes axes={geometry.axes} />
      <CandleNav
        data={props.data}
        label={props.label}
        selected={active}
        onSelect={setFocused}
      />
      <p className="text-body-sm text-ink-muted status-sentence">
        {text("ds.candleOrder", "ko")}
      </p>
      <CandleTable
        data={props.data}
        label={props.label}
        onHighlight={(p): void => {
          setHovered(p?.id);
        }}
      />
    </>
  );
};
export const CandleChart = (props: CandleChartProps): ReactElement => {
  const g = candleCoordinates(props.data);
  const emptyTitle =
    g.kind === "empty" ? props.empty : text("ds.chartInvalid", "ko");
  return (
    <figure
      aria-label={props.label}
      data-chart-theme={props.theme}
      className="@container w-full max-w-full bg-surface text-ink"
    >
      {g.kind === "ready" ? (
        <CandleContent props={props} geometry={g} />
      ) : (
        <EmptyState title={emptyTitle} />
      )}
    </figure>
  );
};
