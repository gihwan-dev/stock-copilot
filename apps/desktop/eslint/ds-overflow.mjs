import { CHART_TABLE_FILES } from "./ds-allowlists.mjs";
import { utilityPattern } from "./ds-overflow-utilities.mjs";
const ds = (names) => names.map((name) => `src/shared/ds/${name}.tsx`);
export const OVERFLOW_FILES = ds(["ScrollArea", "Text", "Heading"]);
export const SCROLL_FILES = ds(["ScrollArea"]);
export const POSITION_FILES = ds([
  "Dialog",
  "Popover",
  "Menu",
  "Tooltip",
  "Toast",
  "AppShell",
  "Table",
]);
export const LENGTH_FILES = ds([
  "Button",
  "button-variants",
  "IconButton",
  "TextField",
  "Select",
  "Combobox",
  "Switch",
  "Checkbox",
  "SegmentedControl",
  "Table",
  "ChartFrame",
  "Toolbar",
  "StatusBar",
  "AppShell",
  "Dialog",
  "ChatInput",
  "Disclosure",
]);
export const VIEWPORT_FILES = ds(["AppShell", "StepScreen"]);
export const NOWRAP_FILES = ds([
  "Numbers",
  "Table",
  "Badge",
  "Button",
  "button-variants",
  "IconButton",
]);
export const OFFSET_FILES = [];
export const PROSE_FILE = "src/shared/ds/Markdown.tsx";
export const LENGTH_EXCEPTIONS = [
  [PROSE_FILE, ["markdown-content", "max-w-prose-max"]],
  ["src/shared/ds/EmptyState.tsx", ["max-w-dialog-width"]],
  ["src/shared/ds/Card.tsx", ["min-h-toolbar-height"]],
  [
    "src/shared/ds/Icon.tsx",
    ["size-icon-sm", "size-icon-md", "size-icon-lg", "size-space-3"],
  ],
  [
    "src/shared/ds/ScrollArea.tsx",
    ["scroll-track-x", "scroll-track-y", "scroll-content-y"],
  ],
  [
    "src/shared/ds/Skeleton.tsx",
    [
      "skeleton-line",
      "h-chart-height-sm",
      "h-chart-height",
      "h-chart-height-lg",
    ],
  ],
  ["src/shared/ds/CandleChart.tsx", ["h-chart-height"]],
  [
    "src/shared/ds/Page.tsx",
    ["min-h-toolbar-height", "max-w-content-max", "wrap-slot"],
  ],
  [
    "src/shared/ds/Split.tsx",
    [
      "w-sidebar-width",
      "w-sidebar-collapsed",
      "w-chat-width",
      "w-chat-width-min",
      "wrap-slot",
    ],
  ],
  [
    "src/shared/ds/Divider.tsx",
    ["h-border-hairline", "w-border-hairline", "min-h-control-height-sm"],
  ],
  ["src/shared/ds/StepScreen.tsx", ["max-w-dialog-width", "h-screen"]],
];
const lengthException = (file) =>
  LENGTH_EXCEPTIONS.find(([path]) => path === file)?.[1] ?? [];
export const OVERFLOW_POLICIES = [
  [
    18,
    OVERFLOW_FILES,
    "(?:overflow(?:-[xy])?-.+|truncate|line-clamp-.+|text-ellipsis)",
  ],
  [19, SCROLL_FILES, "overflow(?:-[xy])?-(?:auto|scroll|overlay)"],
  [
    20,
    POSITION_FILES,
    "(?:absolute|fixed|sticky|(?:-?(?:inset(?:-[xy])?|top|bottom|left|right|start|end))-.+|sr-only)",
  ],
  [
    21,
    LENGTH_FILES,
    "(?:(?:min-|max-)?(?:h|w|size)-(?!(?:auto|full|min|max|fit|0)$).+)",
  ],
  [
    22,
    VIEWPORT_FILES,
    "(?:(?:min-|max-)?(?:h|w|size)-(?:screen|[sdl]?v(?:[hwib]|min|max))|.*(?:\\d|\\.)[sdl]?v(?:[hwib]|min|max).*)",
  ],
  [23, NOWRAP_FILES, "(?:whitespace-(?:nowrap|pre)|text-nowrap)"],
  [
    24,
    OFFSET_FILES,
    "(?:-m[xytrblse]?-.+|-space-[xy]-.+|-?(?:translate|scale)(?:-[xyz])?-.+)",
  ],
];
export const overflowRestrictions = (file) =>
  OVERFLOW_POLICIES.filter(([, files]) => !files.includes(file)).map(
    ([id, , pattern]) => ({
      pattern: `(^|:)!?(?:${id === 21 && lengthException(file).length > 0 ? `(?!(?:${lengthException(file).join("|")})!?$)` : id === 20 && CHART_TABLE_FILES.includes(file) ? "(?!(?:sr-only)!?$)" : ""}(?:${pattern}${utilityPattern(id) ? `|${utilityPattern(id)}` : ""}))!?$`,
      message: `스크롤·넘침 #${id}: 지정 파일에서만 쓴다.`,
    }),
  );

export const overflowKeywords = (file) => [
  ...(OVERFLOW_FILES.includes(file)
    ? [
        "overflow-hidden",
        "overflow-clip",
        "overflow-visible",
        "overflow-x-hidden",
        "overflow-y-hidden",
        "truncate",
        "text-ellipsis",
      ]
    : []),
  ...(SCROLL_FILES.includes(file)
    ? [
        "overflow-auto",
        "overflow-scroll",
        "overflow-x-auto",
        "overflow-y-auto",
        "overflow-x-scroll",
        "overflow-y-scroll",
      ]
    : []),
  ...(POSITION_FILES.includes(file) ? ["absolute", "fixed", "sticky"] : []),
  ...(VIEWPORT_FILES.includes(file)
    ? [
        "h-screen",
        "w-screen",
        "h-dvh",
        "h-svh",
        "h-lvh",
        "w-dvw",
        "w-svw",
        "w-lvw",
      ]
    : []),
];
