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
  "ChartTable",
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
  "Icon",
  "Disclosure",
  "Card",
]);
export const VIEWPORT_FILES = ds(["AppShell"]);
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
    "(?:absolute|fixed|sticky|(?:-?(?:inset(?:-[xy])?|top|bottom|left|right|start|end))-.+|dialog-overlay|sr-only)",
  ],
  [
    21,
    LENGTH_FILES,
    "(?:(?:min-|max-)?(?:h|w|size)-(?!(?:auto|full|min|max|fit|0)$).+|skeleton-line|chat-input|markdown-content)",
  ],
  [
    22,
    VIEWPORT_FILES,
    "(?:(?:min-|max-)?(?:h|w|size)-screen|.+(?:v[hw]|vmin|vmax).*)",
  ],
  [23, NOWRAP_FILES, "whitespace-nowrap"],
  [
    24,
    OFFSET_FILES,
    "(?:-m[xytrblse]?-.+|-?(?:translate|scale)(?:-[xyz])?-.+)",
  ],
];
export const overflowRestrictions = (file) =>
  OVERFLOW_POLICIES.filter(([, files]) => !files.includes(file)).map(
    ([id, , pattern]) => ({
      pattern: `(^|:)!?(?:${id === 21 && file === PROSE_FILE ? "(?!(?:markdown-content|max-w-prose-max)!?$)" : ""}(?:${pattern}))!?$`,
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
  ...(POSITION_FILES.includes(file)
    ? [
        "absolute",
        "fixed",
        "sticky",
        "inset-0",
        "top-0",
        "bottom-0",
        "left-0",
        "right-0",
      ]
    : []),
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
