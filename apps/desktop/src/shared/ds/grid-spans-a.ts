export type GridSpan = 3 | 4 | 6 | 8 | 9 | 12;

export type ResponsiveGridSpan =
  | {
      readonly narrow: GridSpan;
      readonly medium?: GridSpan;
      readonly wide?: GridSpan;
    }
  | {
      readonly narrow?: GridSpan;
      readonly medium: GridSpan;
      readonly wide?: GridSpan;
    }
  | {
      readonly narrow?: GridSpan;
      readonly medium?: GridSpan;
      readonly wide: GridSpan;
    };

export type SpanPair =
  | "3-3"
  | "3-4"
  | "3-6"
  | "3-8"
  | "3-9"
  | "3-12"
  | "4-3"
  | "4-4"
  | "4-6"
  | "4-8"
  | "4-9"
  | "4-12"
  | "6-3"
  | "6-4"
  | "6-6"
  | "6-8"
  | "6-9"
  | "6-12"
  | "8-3"
  | "8-4"
  | "8-6"
  | "8-8"
  | "8-9"
  | "8-12"
  | "9-3"
  | "9-4"
  | "9-6"
  | "9-8"
  | "9-9"
  | "9-12"
  | "12-3"
  | "12-4"
  | "12-6"
  | "12-8"
  | "12-9"
  | "12-12";

export const spans3 = {
  "3-3": "scroll-flex col-span-3",
  "3-4": "scroll-flex col-span-3 @content-wide:col-span-4",
  "3-6": "scroll-flex col-span-3 @content-wide:col-span-6",
  "3-8": "scroll-flex col-span-3 @content-wide:col-span-8",
  "3-9": "scroll-flex col-span-3 @content-wide:col-span-9",
  "3-12": "scroll-flex col-span-3 @content-wide:col-span-12",
  "4-3":
    "scroll-flex col-span-3 @content-medium:col-span-4 @content-wide:col-span-3",
  "4-4": "scroll-flex col-span-3 @content-medium:col-span-4",
  "4-6":
    "scroll-flex col-span-3 @content-medium:col-span-4 @content-wide:col-span-6",
  "4-8":
    "scroll-flex col-span-3 @content-medium:col-span-4 @content-wide:col-span-8",
  "4-9":
    "scroll-flex col-span-3 @content-medium:col-span-4 @content-wide:col-span-9",
  "4-12":
    "scroll-flex col-span-3 @content-medium:col-span-4 @content-wide:col-span-12",
  "6-3":
    "scroll-flex col-span-3 @content-medium:col-span-6 @content-wide:col-span-3",
  "6-4":
    "scroll-flex col-span-3 @content-medium:col-span-6 @content-wide:col-span-4",
  "6-6": "scroll-flex col-span-3 @content-medium:col-span-6",
  "6-8":
    "scroll-flex col-span-3 @content-medium:col-span-6 @content-wide:col-span-8",
  "6-9":
    "scroll-flex col-span-3 @content-medium:col-span-6 @content-wide:col-span-9",
  "6-12":
    "scroll-flex col-span-3 @content-medium:col-span-6 @content-wide:col-span-12",
  "8-3":
    "scroll-flex col-span-3 @content-medium:col-span-8 @content-wide:col-span-3",
  "8-4":
    "scroll-flex col-span-3 @content-medium:col-span-8 @content-wide:col-span-4",
  "8-6":
    "scroll-flex col-span-3 @content-medium:col-span-8 @content-wide:col-span-6",
  "8-8": "scroll-flex col-span-3 @content-medium:col-span-8",
  "8-9":
    "scroll-flex col-span-3 @content-medium:col-span-8 @content-wide:col-span-9",
  "8-12":
    "scroll-flex col-span-3 @content-medium:col-span-8 @content-wide:col-span-12",
  "9-3":
    "scroll-flex col-span-3 @content-medium:col-span-9 @content-wide:col-span-3",
  "9-4":
    "scroll-flex col-span-3 @content-medium:col-span-9 @content-wide:col-span-4",
  "9-6":
    "scroll-flex col-span-3 @content-medium:col-span-9 @content-wide:col-span-6",
  "9-8":
    "scroll-flex col-span-3 @content-medium:col-span-9 @content-wide:col-span-8",
  "9-9": "scroll-flex col-span-3 @content-medium:col-span-9",
  "9-12":
    "scroll-flex col-span-3 @content-medium:col-span-9 @content-wide:col-span-12",
  "12-3":
    "scroll-flex col-span-3 @content-medium:col-span-12 @content-wide:col-span-3",
  "12-4":
    "scroll-flex col-span-3 @content-medium:col-span-12 @content-wide:col-span-4",
  "12-6":
    "scroll-flex col-span-3 @content-medium:col-span-12 @content-wide:col-span-6",
  "12-8":
    "scroll-flex col-span-3 @content-medium:col-span-12 @content-wide:col-span-8",
  "12-9":
    "scroll-flex col-span-3 @content-medium:col-span-12 @content-wide:col-span-9",
  "12-12": "scroll-flex col-span-3 @content-medium:col-span-12",
} as const;

export const spans4 = {
  "3-3": "scroll-flex col-span-4 @content-medium:col-span-3",
  "3-4":
    "scroll-flex col-span-4 @content-medium:col-span-3 @content-wide:col-span-4",
  "3-6":
    "scroll-flex col-span-4 @content-medium:col-span-3 @content-wide:col-span-6",
  "3-8":
    "scroll-flex col-span-4 @content-medium:col-span-3 @content-wide:col-span-8",
  "3-9":
    "scroll-flex col-span-4 @content-medium:col-span-3 @content-wide:col-span-9",
  "3-12":
    "scroll-flex col-span-4 @content-medium:col-span-3 @content-wide:col-span-12",
  "4-3": "scroll-flex col-span-4 @content-wide:col-span-3",
  "4-4": "scroll-flex col-span-4",
  "4-6": "scroll-flex col-span-4 @content-wide:col-span-6",
  "4-8": "scroll-flex col-span-4 @content-wide:col-span-8",
  "4-9": "scroll-flex col-span-4 @content-wide:col-span-9",
  "4-12": "scroll-flex col-span-4 @content-wide:col-span-12",
  "6-3":
    "scroll-flex col-span-4 @content-medium:col-span-6 @content-wide:col-span-3",
  "6-4":
    "scroll-flex col-span-4 @content-medium:col-span-6 @content-wide:col-span-4",
  "6-6": "scroll-flex col-span-4 @content-medium:col-span-6",
  "6-8":
    "scroll-flex col-span-4 @content-medium:col-span-6 @content-wide:col-span-8",
  "6-9":
    "scroll-flex col-span-4 @content-medium:col-span-6 @content-wide:col-span-9",
  "6-12":
    "scroll-flex col-span-4 @content-medium:col-span-6 @content-wide:col-span-12",
  "8-3":
    "scroll-flex col-span-4 @content-medium:col-span-8 @content-wide:col-span-3",
  "8-4":
    "scroll-flex col-span-4 @content-medium:col-span-8 @content-wide:col-span-4",
  "8-6":
    "scroll-flex col-span-4 @content-medium:col-span-8 @content-wide:col-span-6",
  "8-8": "scroll-flex col-span-4 @content-medium:col-span-8",
  "8-9":
    "scroll-flex col-span-4 @content-medium:col-span-8 @content-wide:col-span-9",
  "8-12":
    "scroll-flex col-span-4 @content-medium:col-span-8 @content-wide:col-span-12",
  "9-3":
    "scroll-flex col-span-4 @content-medium:col-span-9 @content-wide:col-span-3",
  "9-4":
    "scroll-flex col-span-4 @content-medium:col-span-9 @content-wide:col-span-4",
  "9-6":
    "scroll-flex col-span-4 @content-medium:col-span-9 @content-wide:col-span-6",
  "9-8":
    "scroll-flex col-span-4 @content-medium:col-span-9 @content-wide:col-span-8",
  "9-9": "scroll-flex col-span-4 @content-medium:col-span-9",
  "9-12":
    "scroll-flex col-span-4 @content-medium:col-span-9 @content-wide:col-span-12",
  "12-3":
    "scroll-flex col-span-4 @content-medium:col-span-12 @content-wide:col-span-3",
  "12-4":
    "scroll-flex col-span-4 @content-medium:col-span-12 @content-wide:col-span-4",
  "12-6":
    "scroll-flex col-span-4 @content-medium:col-span-12 @content-wide:col-span-6",
  "12-8":
    "scroll-flex col-span-4 @content-medium:col-span-12 @content-wide:col-span-8",
  "12-9":
    "scroll-flex col-span-4 @content-medium:col-span-12 @content-wide:col-span-9",
  "12-12": "scroll-flex col-span-4 @content-medium:col-span-12",
} as const;
