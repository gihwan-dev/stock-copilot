import { readFileSync } from "node:fs";

const tokens = JSON.parse(
  readFileSync(
    new URL("../design-tokens/tokens.json", import.meta.url),
    "utf8",
  ),
);
const combine = (prefixes, names) =>
  prefixes.flatMap((p) => names.map((n) => `${p}-${n}`));
const names = (group) => tokens[group].tokens.map((t) => t.name);
export const DS_KEYWORDS = [
  "block",
  "inline-block",
  "flex",
  "inline-flex",
  "grid",
  "inline-grid",
  "hidden",
  "flex-col",
  "flex-row",
  "flex-none",
  "flex-1",
  "items-center",
  "items-start",
  "items-end",
  "justify-center",
  "justify-between",
  "justify-start",
  "justify-end",
  "place-items-center",
  "w-full",
  "max-w-full",
  "cursor-pointer",
  "cursor-not-allowed",
  "border-solid",
  "border-collapse",
  "bg-transparent",
  "text-left",
  "text-center",
  "text-right",
  "tabular-nums",
  "select-none",
  "transition-colors",
  "outline-solid",
  "@container",
];
export const DS_VARIANTS = [
  "hover",
  "focus",
  "focus-visible",
  "active",
  "disabled",
  "motion-reduce",
  "window-md",
  "window-lg",
  "@content-medium",
  "@content-wide",
];
export const DS_CLASSES = [
  ...new Set([
    ...DS_KEYWORDS,
    ...combine(
      [
        "bg",
        "text",
        "border",
        "fill",
        "stroke",
        "outline",
        "ring",
        "decoration",
      ],
      names("color"),
    ),
    ...combine(
      [
        "p",
        "px",
        "py",
        "pt",
        "pb",
        "pl",
        "pr",
        "ps",
        "pe",
        "m",
        "mx",
        "my",
        "mt",
        "mb",
        "ml",
        "mr",
        "ms",
        "me",
        "gap",
        "gap-x",
        "gap-y",
        "space-x",
        "space-y",
      ],
      names("spacing"),
    ),
    ...[...tokens.size.tokens, ...tokens.layout.tokens].flatMap((t) => {
      if (
        /^(?:border-|focus-offset|bp-|grid-columns|control-pad-sm|gap-hairline)/u.test(
          t.name,
        )
      )
        return [];
      const prefixes = /^(?:icon-|dot-size)/u.test(t.name)
        ? ["w", "h", "size"]
        : /height/u.test(t.name) || /높이/u.test(t.usage)
          ? ["h", "min-h", "max-h"]
          : /width/u.test(t.name) || /폭/u.test(t.usage)
            ? ["w", "min-w", "max-w"]
            : [];
      if (prefixes.length === 0)
        throw new Error(`크기 토큰 축을 분류해야 한다: ${t.name}`);
      return combine(prefixes, [t.name]);
    }),
    ...combine(["px", "pl", "pr", "ps", "pe"], ["control-pad-sm"]),
    ...combine(["p", "gap", "gap-x", "gap-y"], ["gap-hairline"]),
    ...combine(
      ["rounded", "rounded-t", "rounded-b", "rounded-l", "rounded-r"],
      names("radius").map((n) => n.replace(/^radius-/u, "")),
    ),
    ...names("shadow"),
    ...names("duration"),
    ...names("easing"),
    ...names("zIndex"),
    ...names("opacity"),
    ...combine(["font"], Object.keys(tokens.type.families)),
    ...combine(
      ["text"],
      tokens.type.groups.flatMap((g) => g.styles.map((s) => s.name)),
    ),
    ...names("text").map((n) =>
      n.startsWith("text-")
        ? n.replace("text-", "text-size-")
        : n.replace(/^weight-/u, "font-"),
    ),
    "border-hairline",
    "border-focus",
    "outline-focus",
    "outline-offset-focus",
    "grid-cols-layout",
  ]),
].sort();

export const DS_PROBES = [
  "bg-red-500",
  "p-5",
  "text-sm",
  "p-[13px]",
  "bg-[#ff0000]",
  "text-[length:13px]",
  "[color:red]",
  "p-(--x)",
  "bg-(--color-primary)",
  "z-50",
  "opacity-50",
  "duration-150",
  "grid-cols-7",
  "border-2",
  "outline-2",
  "line-clamp-3",
  "rotate-45",
  "w-1/2",
  "from-10%",
  "bg-primary/50",
  "text-primary/[0.3]",
  "dark:bg-primary",
  "-m-space-4",
  "p-sidebar-width",
  "w-space-4",
  "h-screen",
  "rounded-full",
  "absolute",
  "overflow-auto",
];
