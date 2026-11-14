import { readFileSync } from "node:fs";
import { parse } from "postcss";
const css = readFileSync(
  new URL("../../../design/components/bundle.css", import.meta.url),
  "utf8",
);
export const designUsage = [];
parse(css).walkDecls((d) => {
  for (const m of d.value.matchAll(/var\(--([a-z\d-]+)\)/gu))
    designUsage.push({
      token: m[1],
      property: d.prop,
      selector: d.parent.selector ?? d.parent.name,
    });
});
const prefixes = {
  width: ["w"],
  height: ["h"],
  "min-width": ["min-w"],
  "max-width": ["max-w"],
  "min-height": ["min-h"],
  "max-height": ["max-h"],
  padding: ["p", "px", "py", "pt", "pb", "pl", "pr", "ps", "pe"],
  "padding-left": ["pl"],
  "padding-right": ["pr"],
  "padding-top": ["pt"],
  "padding-bottom": ["pb"],
  margin: ["m", "mx", "my", "mt", "mb", "ml", "mr", "ms", "me"],
  "margin-left": ["ml"],
  "margin-right": ["mr"],
  "margin-top": ["mt"],
  "margin-bottom": ["mb"],
  gap: ["gap", "gap-x", "gap-y"],
  top: ["top"],
  bottom: ["bottom"],
  left: ["left"],
  right: ["right"],
};
export const designLengthClasses = (tokens) => {
  const names = new Set(
    [
      ...tokens.size.tokens,
      ...tokens.spacing.tokens,
      ...tokens.layout.tokens,
    ].map((t) => t.name),
  );
  const entries = designUsage
    .filter((r) => names.has(r.token))
    .flatMap((r) =>
      (prefixes[r.property] ?? []).map((p) => ({
        name: `${p}-${r.token}`,
        token: r.token,
        property: r.property,
      })),
    );
  for (const r of designUsage)
    if (
      r.property === "width" &&
      names.has(r.token) &&
      designUsage.some(
        (s) =>
          s.token === r.token &&
          s.selector === r.selector &&
          s.property === "height",
      )
    )
      entries.push({
        name: `size-${r.token}`,
        token: r.token,
        property: "width/height",
      });
  return [...new Map(entries.map((r) => [r.name, r])).values()].sort((a, b) =>
    a.name.localeCompare(b.name),
  );
};
export const iconUtilities = [
  ["stroke-icon", "stroke-width", "var(--border-width-focus)"],
  ["stroke-linecap-round", "stroke-linecap", "round"],
  ["stroke-linejoin-round", "stroke-linejoin", "round"],
];
const declaration = (property) =>
  property === "width/height" ? ["width", "height"] : [property];
export const designUtilities = (tokens) => [
  ["rotate-disclosure", "transform", "rotate(90deg)"],
  ["w-switch", "width", "calc(var(--spacing-icon-md) * 2)"],
  ["border-t-hairline", "border-top-width", "var(--border-width-hairline)"],
  ...iconUtilities,
  ...designLengthClasses(tokens)
    .filter((r) => r.token.startsWith("border-") || r.token === "focus-offset")
    .flatMap((r) =>
      declaration(r.property).map((p) => [
        r.name,
        p,
        `var(${r.token === "focus-offset" ? "--sc-focus-offset" : `--border-width-${r.token.slice(7)}`})`,
      ]),
    ),
];
export const themeSource = (tokens) =>
  '@import "tailwindcss" source(none);\n@import "./tokens.css";\n@source "../../";\n\n' +
  designUtilities(tokens)
    .map(
      ([name, property, value]) =>
        `@utility ${name} {\n  ${property}: ${value};\n}`,
    )
    .join("\n\n") +
  "\n";
