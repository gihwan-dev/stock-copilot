import type { SizeToken } from "./tokens";
import { useId, useState, type ReactElement } from "react";
import { text } from "../lib";
import type { ChartProps, ChartGeometry, ChartMark } from "./chart-types";
import { chartNavigation } from "../platform/input";
import { EmptyState } from "./EmptyState";
import { ChartTable, ChartAxes } from "./ChartTable";
import { ChartNumber, chartNumberText } from "./Numbers";
type Props = ChartProps & { readonly geometry: ChartGeometry };
type Ready = ChartProps & {
  readonly geometry: Extract<ChartGeometry, { readonly kind: "ready" }>;
};
export const ChartFrame = (props: Props): ReactElement => (
  <figure
    aria-label={props.label}
    data-chart-theme={props.theme}
    className="@container w-full max-w-full bg-surface text-ink"
  >
    {props.geometry.kind === "ready" ? (
      <ChartContent
        data={props.data}
        label={props.label}
        empty={props.empty}
        theme={props.theme}
        geometry={props.geometry}
      />
    ) : (
      <EmptyState
        title={
          props.geometry.kind === "empty"
            ? props.empty
            : text("ds.chartInvalid", "ko")
        }
      />
    )}
  </figure>
);
const ChartContent = (props: Ready): ReactElement => {
  const [focused, setFocused] = useState<ChartMark | undefined>(undefined);
  const [hovered, setHovered] = useState<ChartMark | undefined>(undefined);
  const focusCurrent = props.geometry.marks.find(
    (mark) => mark.id === focused?.id,
  );
  const hoverCurrent = props.geometry.marks.find(
    (mark) => mark.id === hovered?.id,
  );
  const active = focusCurrent ?? hoverCurrent;
  return (
    <>
      <ChartPlot
        geometry={props.geometry}
        label={props.label}
        active={active}
        onHover={setHovered}
      />
      {props.geometry.axes === undefined ? null : (
        <ChartAxes axes={props.geometry.axes} />
      )}
      <ChartNavigation
        marks={props.geometry.marks}
        label={props.label}
        selected={active}
        onSelect={setFocused}
      />
      <p className="text-body-sm text-ink-muted status-sentence">
        {text(orderMessages[props.geometry.layout], "ko")}
      </p>
      <ChartTable
        data={props.data}
        label={props.label}
        empty={props.empty}
        theme={props.theme}
        layout={props.geometry.layout}
        onHighlight={(point): void => {
          setHovered(
            props.geometry.marks.find((mark) => mark.id === point?.id),
          );
        }}
      />
    </>
  );
};
type NavigationProps = {
  readonly marks: readonly ChartMark[];
  readonly label: ChartProps["label"];
  readonly selected: ChartMark | undefined;
  readonly onSelect: (point: ChartMark | undefined) => void;
};
const move = (
  props: NavigationProps,
  direction: "next" | "previous" | "first" | "last",
): ChartMark | undefined => {
  const index = props.marks.findIndex((mark) => mark.id === props.selected?.id);
  if (direction === "first") return props.marks.at(0);
  if (direction === "last") return props.marks.at(-1);
  if (direction === "next")
    return props.marks.at(index + 1) ?? props.marks.at(0);
  return props.marks.at(index <= 0 ? -1 : index - 1);
};
const ChartNavigation = (props: NavigationProps): ReactElement => {
  const id = useId();
  return (
    <>
      <button
        type="button"
        aria-describedby={id}
        className="text-body-sm text-primary rounded-sm px-space-2 py-space-1 focus-visible:outline-focus"
        onFocus={(): void => {
          props.onSelect(props.selected ?? props.marks.at(0));
        }}
        onBlur={(): void => {
          props.onSelect(undefined);
        }}
        onKeyDown={(event): void => {
          const direction = chartNavigation(event);
          if (direction !== "ignore") props.onSelect(move(props, direction));
        }}
      >
        {text("ds.chartExplore", "ko")}
        {text("ds.chartSeparator", "ko")}
        {props.label}
      </button>
      <p id={id} role="status" className="text-body-sm status-sentence">
        {props.selected === undefined ? null : (
          <>
            {props.selected.label}
            {text("ds.chartSeparator", "ko")}
            <ChartNumber value={props.selected.value} />
          </>
        )}
      </p>
    </>
  );
};
type PlotProps = {
  readonly geometry: Ready["geometry"];
  readonly label: ChartProps["label"];
  readonly active: ChartMark | undefined;
  readonly onHover: (mark: ChartMark | undefined) => void;
};
const ChartPlot = (props: PlotProps): ReactElement => (
  <svg
    role="img"
    aria-label={props.label}
    viewBox="-1 -1 102 62"
    preserveAspectRatio={
      props.geometry.layout === "donut" ? "xMidYMid meet" : "none"
    }
    className="w-full h-chart-height fill-none text-chart-1 stroke-icon"
  >
    {props.geometry.area === undefined ? null : (
      <path d={props.geometry.area} className="fill-chart-1 opacity-area" />
    )}
    {props.geometry.path === undefined ? null : (
      <path
        d={props.geometry.path}
        vectorEffect="non-scaling-stroke"
        className="stroke-current"
      />
    )}
    <ChartMarks
      marks={props.geometry.marks}
      mode={props.geometry.layout === "trend" ? "trend" : "parts"}
      onHover={props.onHover}
    />
    {props.active === undefined ? null : (
      <path
        d={props.active.path}
        pointerEvents="none"
        vectorEffect="non-scaling-stroke"
        className={
          props.geometry.layout === "trend"
            ? "fill-none stroke-focus-ring"
            : "fill-none stroke-surface"
        }
      />
    )}
  </svg>
);
const ChartMarks = (props: {
  readonly marks: readonly ChartMark[];
  readonly mode: "trend" | "parts";
  readonly onHover: PlotProps["onHover"];
}): ReactElement => (
  <>
    {props.marks.map((mark) => (
      <path
        key={mark.id}
        d={mark.path}
        pointerEvents="all"
        className={
          props.mode === "trend" ? "fill-none" : partClasses[mark.order]
        }
        onPointerEnter={(): void => {
          props.onHover(mark);
        }}
        onPointerLeave={(): void => {
          props.onHover(undefined);
        }}
      >
        <title>
          {mark.label}
          {text("ds.chartSeparator", "ko")}
          {chartNumberText(mark.value)}
        </title>
      </path>
    ))}
  </>
);
const partClasses = {
  first: "fill-chart-1",
  second: "fill-chart-2",
  third: "fill-chart-3",
  fourth: "fill-chart-4",
  fifth: "fill-chart-5",
  sixth: "fill-chart-6",
} as const;
const orderMessages = {
  trend: "ds.chartTrendOrder",
  donut: "ds.chartDonutOrder",
  treemap: "ds.chartTreemapOrder",
} as const;
type PlaceholderProps = {
  readonly height: Extract<
    SizeToken,
    "chart-height-sm" | "chart-height" | "chart-height-lg"
  >;
};
export const ChartPlaceholder = (props: PlaceholderProps): ReactElement => {
  if (props.height === "chart-height")
    return (
      <div
        aria-hidden="true"
        className="rounded-md bg-surface-sunken h-chart-height"
      />
    );
  if (props.height === "chart-height-lg")
    return (
      <div
        aria-hidden="true"
        className="rounded-md bg-surface-sunken h-chart-height-lg"
      />
    );
  return (
    <div
      aria-hidden="true"
      className="rounded-md bg-surface-sunken h-chart-height-sm"
    />
  );
};
