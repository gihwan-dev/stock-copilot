import js from "@eslint/js";
import eslintComments from "@eslint-community/eslint-plugin-eslint-comments";
import { defineConfig } from "eslint/config";
import { bans } from "./eslint/bans.mjs";
import { guard } from "./eslint/guard-plugin.mjs";
import { local } from "./eslint/local-plugin.mjs";

const nodeGlobals = Object.fromEntries(
  [
    "process",
    "console",
    "URL",
    "Buffer",
    "setTimeout",
    "clearTimeout",
    "structuredClone",
  ].map((name) => [name, "readonly"]),
);

export const SCRIPT_FILES = [
  "apps/desktop/tests/**/*.mjs",
  "apps/desktop/eslint/**/*.mjs",
  "apps/desktop/*.mjs",
  "harness/**/*.mjs",
];

export default defineConfig(
  {
    ignores: ["**/node_modules/**", "harness/eslint-violations/src/**"],
  },
  {
    linterOptions: {
      noInlineConfig: false,
      reportUnusedDisableDirectives: "error",
      reportUnusedInlineConfigs: "error",
    },
  },
  {
    files: SCRIPT_FILES,
    extends: [js.configs.recommended],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
      globals: nodeGlobals,
    },
    plugins: {
      guard,
      local,
      "@eslint-community/eslint-comments": eslintComments,
    },
    rules: {
      "no-unused-vars": ["error", { ignoreRestSiblings: true }],
      "guard/no-test-skip": ["error", ...bans["guard/no-test-skip"]],
      "guard/disable-directive-form": "error",
      "local/no-comments": "error",
      "@eslint-community/eslint-comments/no-use": [
        "error",
        { allow: ["eslint-disable-next-line"] },
      ],
      "@eslint-community/eslint-comments/require-description": [
        "error",
        { ignore: [] },
      ],
      "@eslint-community/eslint-comments/no-unlimited-disable": "error",
      "@eslint-community/eslint-comments/no-duplicate-disable": "error",
      "@eslint-community/eslint-comments/no-restricted-disable": [
        "error",
        "guard/no-test-skip",
        "guard/disable-directive-form",
        "local/no-comments",
        "@eslint-community/eslint-comments/no-use",
        "@eslint-community/eslint-comments/require-description",
        "@eslint-community/eslint-comments/no-unlimited-disable",
        "@eslint-community/eslint-comments/no-duplicate-disable",
        "@eslint-community/eslint-comments/no-restricted-disable",
      ],
    },
  },
);
