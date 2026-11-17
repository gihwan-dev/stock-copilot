import { readFileSync } from "node:fs";
import { designUtilities } from "./ds-design-usage.mjs";
const tokens = JSON.parse(
  readFileSync(
    new URL("../design-tokens/tokens.json", import.meta.url),
    "utf8",
  ),
);
export const utilityDeclarations = designUtilities(tokens);
export const declarationPolicies = (property, value) => [
  ...(/^(?:overflow(?:-[xy])?|text-overflow|(?:-webkit-)?line-clamp)$/u.test(
    property,
  ) ||
  (property === "clip-path" && value !== "none") ||
  (property === "contain" &&
    /(?:^|\s)(?:paint|strict|content)(?:$|\s)/u.test(value))
    ? [18]
    : []),
  ...(/^overflow(?:-[xy])?$/u.test(property) &&
  /(?:^|\s)(?:auto|scroll|overlay)(?:$|\s)/u.test(value)
    ? [19]
    : []),
  ...((property === "position" && /^(?:absolute|fixed|sticky)$/u.test(value)) ||
  /^(?:inset(?:-.+)?|top|bottom|left|right)$/u.test(property)
    ? [20]
    : []),
  ...(/^(?:(?:min|max)-)?(?:width|height|inline-size|block-size)$/u.test(
    property,
  ) && !/^(?:0|auto|100%|min-content|max-content|fit-content)$/u.test(value)
    ? [21]
    : []),
  ...(/(?:\d|\.)[sdl]?v(?:[hwib]|min|max)/iu.test(value) ? [22] : []),
  ...((property === "white-space" && /^(?:nowrap|pre)$/u.test(value)) ||
  (["text-wrap", "text-wrap-mode"].includes(property) && value === "nowrap")
    ? [23]
    : []),
  ...((/^margin(?:-.+)?$/u.test(property) && /(?:^|\s)-/u.test(value)) ||
  (property === "text-indent" && /^-/u.test(value)) ||
  /^(?:translate|scale)$/u.test(property) ||
  (property === "transform" &&
    /(?:translate|scale|matrix)(?:[XYZ]|3d)?\(/iu.test(value))
    ? [24]
    : []),
];
const escape = (value) => value.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
export const utilityPattern = (id) =>
  [
    ...new Set(
      utilityDeclarations
        .filter(([, property, value]) =>
          declarationPolicies(property, value).includes(id),
        )
        .map(([name]) => escape(name)),
    ),
  ].join("|");
