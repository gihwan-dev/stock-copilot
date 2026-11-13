import * as defaultLists from "./ds-allowlists.mjs";
import { join } from "node:path";
import tailwind from "eslint-plugin-better-tailwindcss";
import tseslint from "typescript-eslint";
import { DS_CLASSES, DS_VARIANTS } from "./ds-classes.mjs";
import { dsPlugin } from "./ds-plugin.mjs";

const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
export const DS_ENTRY = join(import.meta.dirname, "../src/shared/ds/theme.css");
export const DS_RULES = {
  "better-tailwindcss/no-unknown-classes": "error",
  "better-tailwindcss/no-concatenated-classes": "error",
  "better-tailwindcss/no-restricted-classes": [
    "error",
    {
      restrict: [
        {
          pattern: `^(?!(?:(?:${[...DS_VARIANTS, "dark"].map(escape).join("|")}):)*(?:${DS_CLASSES.map(escape).join("|")})$).+$`,
          message: "허용된 토큰 유틸리티와 키워드만 쓴다.",
        },
        {
          pattern: "(^|:)dark:",
          message: "dark 변형 대신 data-theme 토큰을 쓴다.",
        },
        {
          pattern:
            "(^|:)(text|bg|border|fill|stroke|outline|ring|decoration)-(up|down|on-up|on-down)(-|$)",
          message: "가격색은 가격 파일 허용 목록에서만 쓴다.",
        },
      ],
    },
  ],
  "ds/no-dom-styling": "error",
  "ds/no-element-factory": "error",
  "ds/no-style-object": "error",
  "ds/no-presentation-props": "error",
  "ds/static-classes": "error",
  "ds/no-inline-style": "error",
  "ds/no-style-literals": "error",
};
export const SHARED_RULES = {
  "ds/no-dom-styling": "error",
  "ds/no-shared-jsx": "error",
  "ds/no-element-factory": "error",
};
const listed = (files, rules) => (files.length === 0 ? [] : [{ files, rules }]);
export const dsConfigs = (rules = DS_RULES, lists = defaultLists) => [
  {
    files: ["src/shared/ds/**/*.ts", "src/shared/ds/**/*.tsx"],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    plugins: { "better-tailwindcss": tailwind, ds: dsPlugin },
    settings: {
      "better-tailwindcss": {
        cwd: new URL("../", import.meta.url).pathname,
        entryPoint: DS_ENTRY,
      },
    },
    rules,
  },
  {
    files: ["src/shared/**/*.ts", "src/shared/**/*.tsx"],
    ignores: ["src/shared/ds/**"],
    languageOptions: {
      parser: tseslint.parser,
      parserOptions: { ecmaFeatures: { jsx: true } },
    },
    plugins: { ds: dsPlugin },
    rules: SHARED_RULES,
  },
  ...listed(lists.PRICE_COLOR_FILES, {
    "better-tailwindcss/no-restricted-classes": [
      "error",
      {
        restrict: DS_RULES[
          "better-tailwindcss/no-restricted-classes"
        ][1].restrict.slice(0, 2),
      },
    ],
  }),
  ...listed(lists.INLINE_STYLE_FILES, { "ds/no-inline-style": "off" }),
  ...listed(lists.DOM_STYLE_FILES, { "ds/no-dom-styling": "off" }),
];
