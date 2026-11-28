import { SectionTarget, type SectionPosition } from "./SectionTarget";
import type { ReactElement } from "react";
import { LayoutItems, type LayoutChildren } from "./layout-children";
import type { SpacingToken } from "./tokens";

type GapKey =
  | "space-1"
  | "space-2"
  | "space-3"
  | "space-4"
  | "space-5"
  | "space-6"
  | "space-7"
  | "space-8"
  | "space-9"
  | "space-10"
  | "space-11"
  | "space-12";

type Props = LayoutChildren & {
  readonly position?: SectionPosition;
  readonly gap?: SpacingToken;
};

const stackClasses = {
  "space-1": "flex flex-col gap-space-1 w-full max-w-full scroll-flex",
  "space-2": "flex flex-col gap-space-2 w-full max-w-full scroll-flex",
  "space-3": "flex flex-col gap-space-3 w-full max-w-full scroll-flex",
  "space-4": "flex flex-col gap-space-4 w-full max-w-full scroll-flex",
  "space-5": "flex flex-col gap-space-5 w-full max-w-full scroll-flex",
  "space-6": "flex flex-col gap-space-6 w-full max-w-full scroll-flex",
  "space-7": "flex flex-col gap-space-7 w-full max-w-full scroll-flex",
  "space-8": "flex flex-col gap-space-8 w-full max-w-full scroll-flex",
  "space-9": "flex flex-col gap-space-9 w-full max-w-full scroll-flex",
  "space-10": "flex flex-col gap-space-10 w-full max-w-full scroll-flex",
  "space-11": "flex flex-col gap-space-11 w-full max-w-full scroll-flex",
  "space-12": "flex flex-col gap-space-12 w-full max-w-full scroll-flex",
} as const;

const StackBox = (
  props: LayoutChildren & {
    readonly track: GapKey;
  },
): ReactElement => (
  <div className={stackClasses[props.track]}>
    <LayoutItems mode="block">{props.children}</LayoutItems>
  </div>
);

export const Stack = (props: Props): ReactElement => (
  <SectionTarget position={props.position}>
    <StackBox track={props.gap ?? "space-4"}>{props.children}</StackBox>
  </SectionTarget>
);
