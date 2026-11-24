export type InlineAlign = "start" | "center" | "end";
export type InlineJustify = "start" | "center" | "end" | "between";
export type InlineWrap = "wrap" | "nowrap";

export type InlineLayoutKey =
  | "start-start-wrap"
  | "start-start-nowrap"
  | "start-center-wrap"
  | "start-center-nowrap"
  | "start-end-wrap"
  | "start-end-nowrap"
  | "start-between-wrap"
  | "start-between-nowrap"
  | "center-start-wrap"
  | "center-start-nowrap"
  | "center-center-wrap"
  | "center-center-nowrap"
  | "center-end-wrap"
  | "center-end-nowrap"
  | "center-between-wrap"
  | "center-between-nowrap"
  | "end-start-wrap"
  | "end-start-nowrap"
  | "end-center-wrap"
  | "end-center-nowrap"
  | "end-end-wrap"
  | "end-end-nowrap"
  | "end-between-wrap"
  | "end-between-nowrap";

export const inlineSpace1 = {
  "start-start-wrap":
    "flex flex-row flex-wrap items-start justify-start gap-space-1 w-full max-w-full scroll-flex",
  "start-start-nowrap":
    "flex flex-row items-start justify-start gap-space-1 w-full max-w-full scroll-flex",
  "start-center-wrap":
    "flex flex-row flex-wrap items-start justify-center gap-space-1 w-full max-w-full scroll-flex",
  "start-center-nowrap":
    "flex flex-row items-start justify-center gap-space-1 w-full max-w-full scroll-flex",
  "start-end-wrap":
    "flex flex-row flex-wrap items-start justify-end gap-space-1 w-full max-w-full scroll-flex",
  "start-end-nowrap":
    "flex flex-row items-start justify-end gap-space-1 w-full max-w-full scroll-flex",
  "start-between-wrap":
    "flex flex-row flex-wrap items-start justify-between gap-space-1 w-full max-w-full scroll-flex",
  "start-between-nowrap":
    "flex flex-row items-start justify-between gap-space-1 w-full max-w-full scroll-flex",
  "center-start-wrap":
    "flex flex-row flex-wrap items-center justify-start gap-space-1 w-full max-w-full scroll-flex",
  "center-start-nowrap":
    "flex flex-row items-center justify-start gap-space-1 w-full max-w-full scroll-flex",
  "center-center-wrap":
    "flex flex-row flex-wrap items-center justify-center gap-space-1 w-full max-w-full scroll-flex",
  "center-center-nowrap":
    "flex flex-row items-center justify-center gap-space-1 w-full max-w-full scroll-flex",
  "center-end-wrap":
    "flex flex-row flex-wrap items-center justify-end gap-space-1 w-full max-w-full scroll-flex",
  "center-end-nowrap":
    "flex flex-row items-center justify-end gap-space-1 w-full max-w-full scroll-flex",
  "center-between-wrap":
    "flex flex-row flex-wrap items-center justify-between gap-space-1 w-full max-w-full scroll-flex",
  "center-between-nowrap":
    "flex flex-row items-center justify-between gap-space-1 w-full max-w-full scroll-flex",
  "end-start-wrap":
    "flex flex-row flex-wrap items-end justify-start gap-space-1 w-full max-w-full scroll-flex",
  "end-start-nowrap":
    "flex flex-row items-end justify-start gap-space-1 w-full max-w-full scroll-flex",
  "end-center-wrap":
    "flex flex-row flex-wrap items-end justify-center gap-space-1 w-full max-w-full scroll-flex",
  "end-center-nowrap":
    "flex flex-row items-end justify-center gap-space-1 w-full max-w-full scroll-flex",
  "end-end-wrap":
    "flex flex-row flex-wrap items-end justify-end gap-space-1 w-full max-w-full scroll-flex",
  "end-end-nowrap":
    "flex flex-row items-end justify-end gap-space-1 w-full max-w-full scroll-flex",
  "end-between-wrap":
    "flex flex-row flex-wrap items-end justify-between gap-space-1 w-full max-w-full scroll-flex",
  "end-between-nowrap":
    "flex flex-row items-end justify-between gap-space-1 w-full max-w-full scroll-flex",
} as const;

export const inlineSpace2 = {
  "start-start-wrap":
    "flex flex-row flex-wrap items-start justify-start gap-space-2 w-full max-w-full scroll-flex",
  "start-start-nowrap":
    "flex flex-row items-start justify-start gap-space-2 w-full max-w-full scroll-flex",
  "start-center-wrap":
    "flex flex-row flex-wrap items-start justify-center gap-space-2 w-full max-w-full scroll-flex",
  "start-center-nowrap":
    "flex flex-row items-start justify-center gap-space-2 w-full max-w-full scroll-flex",
  "start-end-wrap":
    "flex flex-row flex-wrap items-start justify-end gap-space-2 w-full max-w-full scroll-flex",
  "start-end-nowrap":
    "flex flex-row items-start justify-end gap-space-2 w-full max-w-full scroll-flex",
  "start-between-wrap":
    "flex flex-row flex-wrap items-start justify-between gap-space-2 w-full max-w-full scroll-flex",
  "start-between-nowrap":
    "flex flex-row items-start justify-between gap-space-2 w-full max-w-full scroll-flex",
  "center-start-wrap":
    "flex flex-row flex-wrap items-center justify-start gap-space-2 w-full max-w-full scroll-flex",
  "center-start-nowrap":
    "flex flex-row items-center justify-start gap-space-2 w-full max-w-full scroll-flex",
  "center-center-wrap":
    "flex flex-row flex-wrap items-center justify-center gap-space-2 w-full max-w-full scroll-flex",
  "center-center-nowrap":
    "flex flex-row items-center justify-center gap-space-2 w-full max-w-full scroll-flex",
  "center-end-wrap":
    "flex flex-row flex-wrap items-center justify-end gap-space-2 w-full max-w-full scroll-flex",
  "center-end-nowrap":
    "flex flex-row items-center justify-end gap-space-2 w-full max-w-full scroll-flex",
  "center-between-wrap":
    "flex flex-row flex-wrap items-center justify-between gap-space-2 w-full max-w-full scroll-flex",
  "center-between-nowrap":
    "flex flex-row items-center justify-between gap-space-2 w-full max-w-full scroll-flex",
  "end-start-wrap":
    "flex flex-row flex-wrap items-end justify-start gap-space-2 w-full max-w-full scroll-flex",
  "end-start-nowrap":
    "flex flex-row items-end justify-start gap-space-2 w-full max-w-full scroll-flex",
  "end-center-wrap":
    "flex flex-row flex-wrap items-end justify-center gap-space-2 w-full max-w-full scroll-flex",
  "end-center-nowrap":
    "flex flex-row items-end justify-center gap-space-2 w-full max-w-full scroll-flex",
  "end-end-wrap":
    "flex flex-row flex-wrap items-end justify-end gap-space-2 w-full max-w-full scroll-flex",
  "end-end-nowrap":
    "flex flex-row items-end justify-end gap-space-2 w-full max-w-full scroll-flex",
  "end-between-wrap":
    "flex flex-row flex-wrap items-end justify-between gap-space-2 w-full max-w-full scroll-flex",
  "end-between-nowrap":
    "flex flex-row items-end justify-between gap-space-2 w-full max-w-full scroll-flex",
} as const;

export const inlineSpace3 = {
  "start-start-wrap":
    "flex flex-row flex-wrap items-start justify-start gap-space-3 w-full max-w-full scroll-flex",
  "start-start-nowrap":
    "flex flex-row items-start justify-start gap-space-3 w-full max-w-full scroll-flex",
  "start-center-wrap":
    "flex flex-row flex-wrap items-start justify-center gap-space-3 w-full max-w-full scroll-flex",
  "start-center-nowrap":
    "flex flex-row items-start justify-center gap-space-3 w-full max-w-full scroll-flex",
  "start-end-wrap":
    "flex flex-row flex-wrap items-start justify-end gap-space-3 w-full max-w-full scroll-flex",
  "start-end-nowrap":
    "flex flex-row items-start justify-end gap-space-3 w-full max-w-full scroll-flex",
  "start-between-wrap":
    "flex flex-row flex-wrap items-start justify-between gap-space-3 w-full max-w-full scroll-flex",
  "start-between-nowrap":
    "flex flex-row items-start justify-between gap-space-3 w-full max-w-full scroll-flex",
  "center-start-wrap":
    "flex flex-row flex-wrap items-center justify-start gap-space-3 w-full max-w-full scroll-flex",
  "center-start-nowrap":
    "flex flex-row items-center justify-start gap-space-3 w-full max-w-full scroll-flex",
  "center-center-wrap":
    "flex flex-row flex-wrap items-center justify-center gap-space-3 w-full max-w-full scroll-flex",
  "center-center-nowrap":
    "flex flex-row items-center justify-center gap-space-3 w-full max-w-full scroll-flex",
  "center-end-wrap":
    "flex flex-row flex-wrap items-center justify-end gap-space-3 w-full max-w-full scroll-flex",
  "center-end-nowrap":
    "flex flex-row items-center justify-end gap-space-3 w-full max-w-full scroll-flex",
  "center-between-wrap":
    "flex flex-row flex-wrap items-center justify-between gap-space-3 w-full max-w-full scroll-flex",
  "center-between-nowrap":
    "flex flex-row items-center justify-between gap-space-3 w-full max-w-full scroll-flex",
  "end-start-wrap":
    "flex flex-row flex-wrap items-end justify-start gap-space-3 w-full max-w-full scroll-flex",
  "end-start-nowrap":
    "flex flex-row items-end justify-start gap-space-3 w-full max-w-full scroll-flex",
  "end-center-wrap":
    "flex flex-row flex-wrap items-end justify-center gap-space-3 w-full max-w-full scroll-flex",
  "end-center-nowrap":
    "flex flex-row items-end justify-center gap-space-3 w-full max-w-full scroll-flex",
  "end-end-wrap":
    "flex flex-row flex-wrap items-end justify-end gap-space-3 w-full max-w-full scroll-flex",
  "end-end-nowrap":
    "flex flex-row items-end justify-end gap-space-3 w-full max-w-full scroll-flex",
  "end-between-wrap":
    "flex flex-row flex-wrap items-end justify-between gap-space-3 w-full max-w-full scroll-flex",
  "end-between-nowrap":
    "flex flex-row items-end justify-between gap-space-3 w-full max-w-full scroll-flex",
} as const;
