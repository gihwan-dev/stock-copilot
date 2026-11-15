import { useState, type ReactElement } from "react";
import { text } from "../lib";
import type {
  ChartPoint,
  ChartProps,
  ChartGeometry,
  ChartMark,
} from "./chart-types";
import { EmptyState } from "./EmptyState";
import { ChartTable } from "./ChartTable";
type Props = ChartProps & { readonly geometry: ChartGeometry };
export const ChartFrame = (props: Props): ReactElement => {
  const [focused, setFocused] = useState<ChartPoint | undefined>(undefined);
  const [hovered, setHovered] = useState<ChartPoint | undefined>(undefined);
  const selected = focused ?? hovered;
  const active = props.data.find((point) => point.label === selected?.label);
  if (props.geometry.kind !== "ready")
    return (
      <EmptyState
        title={
          props.geometry.kind === "empty"
            ? props.empty
            : text("ds.chartInvalid", "ko")
        }
      />
    );
  return (
    <figure data-theme={props.theme} className="w-full max-w-full text-ink">
      <svg
        role="img"
        aria-label={props.label}
        viewBox="0 0 100 60"
        className="w-full h-chart-height fill-none stroke-current text-chart-1 stroke-icon"
      >
        {props.geometry.area === undefined ? null : (
          <path d={props.geometry.area} className="fill-chart-1 opacity-area" />
        )}
        {props.geometry.path === undefined ? null : (
          <path d={props.geometry.path} />
        )}
        <ChartMarks
          marks={props.geometry.marks}
          onSelect={setFocused}
          onHover={setHovered}
        />
      </svg>
      <figcaption role="status" className="text-body-sm status-sentence">
        {active === undefined ? null : (
          <>
            {active.label}
            {text("ds.chartSeparator", "ko")}
            {active.value}
          </>
        )}
      </figcaption>
      <ChartTable
        data={props.data}
        label={props.label}
        empty={props.empty}
        theme={props.theme}
      />
    </figure>
  );
};

const ChartMarks = (props: {
  readonly marks: readonly ChartMark[];
  readonly onSelect: (point: ChartPoint | undefined) => void;
  readonly onHover: (point: ChartPoint | undefined) => void;
}): ReactElement => (
  <>
    {props.marks.map((mark) => (
      <path
        key={mark.label}
        d={mark.path}
        tabIndex={0}
        aria-label={mark.label}
        className={partClasses[mark.order]}
        onFocus={(): void => {
          props.onSelect(mark);
        }}
        onBlur={(): void => {
          props.onSelect(undefined);
        }}
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
          {mark.value}
        </title>
      </path>
    ))}
  </>
);

const partClasses = {
  first: "fill-chart-1 stroke-surface focus:stroke-focus-ring",
  second: "fill-chart-2 stroke-surface focus:stroke-focus-ring",
  third: "fill-chart-3 stroke-surface focus:stroke-focus-ring",
  fourth: "fill-chart-4 stroke-surface focus:stroke-focus-ring",
  fifth: "fill-chart-5 stroke-surface focus:stroke-focus-ring",
  sixth: "fill-chart-6 stroke-surface focus:stroke-focus-ring",
} as const;
