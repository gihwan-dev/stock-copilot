import type { ReactElement } from "react";
import {
  spans3,
  spans4,
  type GridSpan,
  type ResponsiveGridSpan,
  type SpanPair,
} from "./grid-spans-a";
import { spans6, spans8 } from "./grid-spans-b";
import { spans9, spans12 } from "./grid-spans-c";
import type { LayoutChildren } from "./layout-children";

type Props = LayoutChildren & {
  readonly span: GridSpan | ResponsiveGridSpan;
};

const wide3 = (wide: GridSpan): SpanPair => {
  switch (wide) {
    case 3:
      return "3-3";
    case 4:
      return "3-4";
    case 6:
      return "3-6";
    case 8:
      return "3-8";
    case 9:
      return "3-9";
    case 12:
      return "3-12";
  }
};

const wide4 = (wide: GridSpan): SpanPair => {
  switch (wide) {
    case 3:
      return "4-3";
    case 4:
      return "4-4";
    case 6:
      return "4-6";
    case 8:
      return "4-8";
    case 9:
      return "4-9";
    case 12:
      return "4-12";
  }
};

const wide6 = (wide: GridSpan): SpanPair => {
  switch (wide) {
    case 3:
      return "6-3";
    case 4:
      return "6-4";
    case 6:
      return "6-6";
    case 8:
      return "6-8";
    case 9:
      return "6-9";
    case 12:
      return "6-12";
  }
};

const wide8 = (wide: GridSpan): SpanPair => {
  switch (wide) {
    case 3:
      return "8-3";
    case 4:
      return "8-4";
    case 6:
      return "8-6";
    case 8:
      return "8-8";
    case 9:
      return "8-9";
    case 12:
      return "8-12";
  }
};

const wide9 = (wide: GridSpan): SpanPair => {
  switch (wide) {
    case 3:
      return "9-3";
    case 4:
      return "9-4";
    case 6:
      return "9-6";
    case 8:
      return "9-8";
    case 9:
      return "9-9";
    case 12:
      return "9-12";
  }
};

const wide12 = (wide: GridSpan): SpanPair => {
  switch (wide) {
    case 3:
      return "12-3";
    case 4:
      return "12-4";
    case 6:
      return "12-6";
    case 8:
      return "12-8";
    case 9:
      return "12-9";
    case 12:
      return "12-12";
  }
};

const pairOf = (medium: GridSpan, wide: GridSpan): SpanPair => {
  switch (medium) {
    case 3:
      return wide3(wide);
    case 4:
      return wide4(wide);
    case 6:
      return wide6(wide);
    case 8:
      return wide8(wide);
    case 9:
      return wide9(wide);
    case 12:
      return wide12(wide);
  }
};

const SpanBox = (
  props: LayoutChildren & {
    readonly narrow: GridSpan;
    readonly pair: SpanPair;
  },
): ReactElement => {
  switch (props.narrow) {
    case 3:
      return <div className={spans3[props.pair]}>{props.children}</div>;
    case 4:
      return <div className={spans4[props.pair]}>{props.children}</div>;
    case 6:
      return <div className={spans6[props.pair]}>{props.children}</div>;
    case 8:
      return <div className={spans8[props.pair]}>{props.children}</div>;
    case 9:
      return <div className={spans9[props.pair]}>{props.children}</div>;
    case 12:
      return <div className={spans12[props.pair]}>{props.children}</div>;
  }
};

export const GridItem = (props: Props): ReactElement => {
  const narrow: GridSpan =
    typeof props.span === "number" ? props.span : (props.span.narrow ?? 12);
  const medium: GridSpan =
    typeof props.span === "number" ? props.span : (props.span.medium ?? narrow);
  const wide: GridSpan =
    typeof props.span === "number" ? props.span : (props.span.wide ?? medium);
  return (
    <SpanBox narrow={narrow} pair={pairOf(medium, wide)}>
      {props.children}
    </SpanBox>
  );
};
