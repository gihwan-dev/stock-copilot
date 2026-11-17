import js from "@eslint/js";
import eslintComments from "@eslint-community/eslint-plugin-eslint-comments";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";
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
export const BROWSER_TEST_FILES = ["apps/desktop/tests/ds20-*.browser.ts"];

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
  {
    files: BROWSER_TEST_FILES,
    languageOptions: {
      parser: tseslint.parser,
      sourceType: "module",
      globals: {
        ...nodeGlobals,
        document: "readonly",
        window: "readonly",
        fetch: "readonly",
        beforeAll: "readonly",
        afterEach: "readonly",
        describe: "readonly",
        it: "readonly",
        expect: "readonly",
      },
    },
    plugins: {
      guard,
      local,
      "@typescript-eslint": tseslint.plugin,
      "@eslint-community/eslint-comments": eslintComments,
    },
    rules: {
      "@typescript-eslint/no-unused-vars": [
        "error",
        { argsIgnorePattern: "^_" },
      ],
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
        "@typescript-eslint/no-unused-vars",
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
