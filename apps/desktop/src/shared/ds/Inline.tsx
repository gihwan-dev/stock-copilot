import type { ReactElement } from "react";
import {
  inlineSpace1,
  inlineSpace2,
  inlineSpace3,
  type InlineAlign,
  type InlineJustify,
  type InlineLayoutKey,
  type InlineWrap,
} from "./inline-classes-a";
import { inlineSpace4, inlineSpace5, inlineSpace6 } from "./inline-classes-b";
import { inlineSpace7, inlineSpace8, inlineSpace9 } from "./inline-classes-c";
import {
  inlineSpace10,
  inlineSpace11,
  inlineSpace12,
} from "./inline-classes-d";
import { LayoutItems, type LayoutChildren } from "./layout-children";
import type { SpacingToken } from "./tokens";

type Props = LayoutChildren & {
  readonly gap?: SpacingToken;
  readonly align?: InlineAlign;
  readonly justify?: InlineJustify;
  readonly wrap?: InlineWrap;
};

type LowGap =
  "space-1" | "space-2" | "space-3" | "space-4" | "space-5" | "space-6";

type HighGap =
  "space-7" | "space-8" | "space-9" | "space-10" | "space-11" | "space-12";

const startWrap = {
  start: "start-start-wrap",
  center: "start-center-wrap",
  end: "start-end-wrap",
  between: "start-between-wrap",
} as const;

const startNowrap = {
  start: "start-start-nowrap",
  center: "start-center-nowrap",
  end: "start-end-nowrap",
  between: "start-between-nowrap",
} as const;

const centerWrap = {
  start: "center-start-wrap",
  center: "center-center-wrap",
  end: "center-end-wrap",
  between: "center-between-wrap",
} as const;

const centerNowrap = {
  start: "center-start-nowrap",
  center: "center-center-nowrap",
  end: "center-end-nowrap",
  between: "center-between-nowrap",
} as const;

const endWrap = {
  start: "end-start-wrap",
  center: "end-center-wrap",
  end: "end-end-wrap",
  between: "end-between-wrap",
} as const;

const endNowrap = {
  start: "end-start-nowrap",
  center: "end-center-nowrap",
  end: "end-end-nowrap",
  between: "end-between-nowrap",
} as const;

const layoutKey = (
  align: InlineAlign,
  justify: InlineJustify,
  wrap: InlineWrap,
): InlineLayoutKey => {
  if (align === "start")
    return wrap === "wrap" ? startWrap[justify] : startNowrap[justify];
  if (align === "center")
    return wrap === "wrap" ? centerWrap[justify] : centerNowrap[justify];
  return wrap === "wrap" ? endWrap[justify] : endNowrap[justify];
};

const gapBand = {
  "space-1": { group: "low", track: "space-1" },
  "space-2": { group: "low", track: "space-2" },
  "space-3": { group: "low", track: "space-3" },
  "space-4": { group: "low", track: "space-4" },
  "space-5": { group: "low", track: "space-5" },
  "space-6": { group: "low", track: "space-6" },
  "space-7": { group: "high", track: "space-7" },
  "space-8": { group: "high", track: "space-8" },
  "space-9": { group: "high", track: "space-9" },
  "space-10": { group: "high", track: "space-10" },
  "space-11": { group: "high", track: "space-11" },
  "space-12": { group: "high", track: "space-12" },
} as const;

const InlineLow = (
  props: LayoutChildren & {
    readonly track: LowGap;
    readonly preset: InlineLayoutKey;
  },
): ReactElement => {
  switch (props.track) {
    case "space-1":
      return (
        <div className={inlineSpace1[props.preset]}>
          <LayoutItems mode="inline">{props.children}</LayoutItems>
        </div>
      );
    case "space-2":
      return (
        <div className={inlineSpace2[props.preset]}>
          <LayoutItems mode="inline">{props.children}</LayoutItems>
        </div>
      );
    case "space-3":
      return (
        <div className={inlineSpace3[props.preset]}>
          <LayoutItems mode="inline">{props.children}</LayoutItems>
        </div>
      );
    case "space-4":
      return (
        <div className={inlineSpace4[props.preset]}>
          <LayoutItems mode="inline">{props.children}</LayoutItems>
        </div>
      );
    case "space-5":
      return (
        <div className={inlineSpace5[props.preset]}>
          <LayoutItems mode="inline">{props.children}</LayoutItems>
        </div>
      );
    case "space-6":
      return (
        <div className={inlineSpace6[props.preset]}>
          <LayoutItems mode="inline">{props.children}</LayoutItems>
        </div>
      );
  }
};

const InlineHigh = (
  props: LayoutChildren & {
    readonly track: HighGap;
    readonly preset: InlineLayoutKey;
  },
): ReactElement => {
  switch (props.track) {
    case "space-7":
      return (
        <div className={inlineSpace7[props.preset]}>
          <LayoutItems mode="inline">{props.children}</LayoutItems>
        </div>
      );
    case "space-8":
      return (
        <div className={inlineSpace8[props.preset]}>
          <LayoutItems mode="inline">{props.children}</LayoutItems>
        </div>
      );
    case "space-9":
      return (
        <div className={inlineSpace9[props.preset]}>
          <LayoutItems mode="inline">{props.children}</LayoutItems>
        </div>
      );
    case "space-10":
      return (
        <div className={inlineSpace10[props.preset]}>
          <LayoutItems mode="inline">{props.children}</LayoutItems>
        </div>
      );
    case "space-11":
      return (
        <div className={inlineSpace11[props.preset]}>
          <LayoutItems mode="inline">{props.children}</LayoutItems>
        </div>
      );
    case "space-12":
      return (
        <div className={inlineSpace12[props.preset]}>
          <LayoutItems mode="inline">{props.children}</LayoutItems>
        </div>
      );
  }
};

export const Inline = (props: Props): ReactElement => {
  const resolved = gapBand[props.gap ?? "space-2"];
  const preset = layoutKey(
    props.align ?? "center",
    props.justify ?? "start",
    props.wrap ?? "nowrap",
  );
  if (resolved.group === "low")
    return (
      <InlineLow track={resolved.track} preset={preset}>
        {props.children}
      </InlineLow>
    );
  return (
    <InlineHigh track={resolved.track} preset={preset}>
      {props.children}
    </InlineHigh>
  );
};
