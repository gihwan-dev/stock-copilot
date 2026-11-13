import {
  dsConfigs,
  DS_RULES,
  SHARED_RULES,
  SCREEN_RULES,
} from "./ds-config.mjs";
import js from "@eslint/js";
import eslintComments from "@eslint-community/eslint-plugin-eslint-comments";
import eslintReact from "@eslint-react/eslint-plugin";
import boundaries from "eslint-plugin-boundaries";
import functional from "eslint-plugin-functional";
import reactHooks from "eslint-plugin-react-hooks";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";
import { join } from "node:path";
import { bans, importBans } from "./bans.mjs";
import {
  BOUNDARY_TABLES,
  boundaryRules,
  boundarySettings,
} from "./boundaries.mjs";
import { guard } from "./guard-plugin.mjs";
import { emptyParser, local } from "./local-plugin.mjs";
import { generatedConfigs } from "./generated.mjs";

const TS_FILES = ["src/**/*.ts", "src/**/*.tsx"];
const SCREEN = ["src/app", "src/features", "src/shared/ds"].flatMap(
  (folder) => [`${folder}/**/*.ts`, `${folder}/**/*.tsx`],
);
const IPC_TESTING = ["src/shared/ipc/testing/**"];
const SHARED = ["src/shared/**"];
const FORBIDDEN_FILES = [
  "src/**/*.js",
  "src/**/*.jsx",
  "src/**/*.mjs",
  "src/**/*.cjs",
  "src/**/*.mts",
  "src/**/*.cts",
  "src/**/*.d.ts",
];
const OTHER_FILES = ["src/**/*.*", "src/**/.*", "src/**/[!.]*"];

const UNDISABLEABLE_RULES = [
  ...new Set([
    ...Object.keys(DS_RULES),
    ...Object.keys(SHARED_RULES),
    ...Object.keys(SCREEN_RULES),
  ]),
  "ds/no-dom-exports",
  "ds/no-message-format",
  "local/no-comments",
  "local/no-money-coercion",
  "local/forbidden-file",
  "local/no-raw-types",
  "local/no-brand-creation",
  "local/no-ui-text",
  "local/no-promise-catch",
  "local/no-computed-member",
  "local/test-file-boundary",
  "local/ipc-testing-boundary",
  "boundaries/*",
  "guard/no-effect-hooks",
  "guard/no-sync-external-store",
  "guard/no-ref-callback",
  "guard/no-type-predicate",
  "guard/no-number-conversion",
  "guard/no-unary-plus",
  "guard/no-reflect",
  "guard/no-prototype",
  "guard/no-call-apply-bind",
  "guard/no-own-symbols",
  "guard/no-network",
  "guard/no-global-object",
  "guard/no-test-skip",
  "guard/no-namespace-import",
  "guard/no-tauri-mocks",
  "guard/forbidden-extension",
  "guard/no-dom-globals",
  "guard/disable-directive-form",
  "guard/no-import-meta",
  "guard/no-jsx-spread",
  "guard/no-module-mocks",
  "@typescript-eslint/ban-ts-comment",
  "@typescript-eslint/consistent-type-assertions",
  "@typescript-eslint/no-unsafe-type-assertion",
  "functional/no-throw-statements",
  "@typescript-eslint/only-throw-error",
  "functional/no-try-statements",
  "functional/no-expression-statements",
  "@eslint-community/eslint-comments/no-use",
  "@eslint-community/eslint-comments/require-description",
  "@eslint-community/eslint-comments/no-unlimited-disable",
  "@eslint-community/eslint-comments/no-restricted-disable",
  "@eslint-community/eslint-comments/no-duplicate-disable",
];

const banRules = Object.fromEntries([
  ...Object.entries(bans).map(([rule, entries]) => [
    rule,
    ["error", ...entries],
  ]),
  ...Object.entries(importBans).map(([rule, options]) => [
    rule,
    ["error", options],
  ]),
]);

const assertionRules = [
  "@typescript-eslint/consistent-type-assertions",
  "@typescript-eslint/no-unsafe-type-assertion",
];

const off = (rules) => Object.fromEntries(rules.map((rule) => [rule, "off"]));

const whenListed = (files, block) =>
  files.length === 0 ? [] : [{ ...block, files }];

const asError = (entry) =>
  entry === "warn" || entry === 1
    ? "error"
    : Array.isArray(entry) && (entry[0] === "warn" || entry[0] === 1)
      ? ["error", ...entry.slice(1)]
      : entry;

const warningsAsErrors = (configs) =>
  configs.map((config) =>
    config.rules === undefined
      ? config
      : {
          ...config,
          rules: Object.fromEntries(
            Object.entries(config.rules).map(([rule, entry]) => [
              rule,
              asError(entry),
            ]),
          ),
        },
  );

export const appConfig = (
  {
    CHART_NUMBER_ALLOWLIST,
    FILE_PLACEHOLDERS,
    IPC_TESTING_FILE_ALLOWLIST,
    MESSAGES_FILES,
    MONEY_FILE,
    MONEY_KEY_READERS,
    PLATFORM_FILE_ALLOWLIST,
    PROMISE_REJECTION_ALLOWLIST,
    RAW_TYPE_ALLOWLIST,
    THROW_ALLOWLIST,
    TYPE_PREDICATE_ALLOWLIST,
  },
  boundaryTables = BOUNDARY_TABLES,
) => {
  const FILE_FOLDER_ALLOWLISTS = [
    { folder: "src/shared/platform", files: PLATFORM_FILE_ALLOWLIST },
    { folder: "src/shared/ipc/testing", files: IPC_TESTING_FILE_ALLOWLIST },
  ];
  const configs = defineConfig(
    {
      ignores: [
        "dist/**",
        "src-tauri/**",
        "e2e-app/**",
        "node_modules/**",
        "!src/**/node_modules/",
      ],
    },
    {
      linterOptions: {
        noInlineConfig: false,
        reportUnusedDisableDirectives: "error",
        reportUnusedInlineConfigs: "error",
      },
    },
    {
      files: TS_FILES,
      extends: [
        js.configs.recommended,
        tseslint.configs.strictTypeChecked,
        tseslint.configs.stylisticTypeChecked,
        functional.configs.externalTypeScriptRecommended,
        functional.configs.recommended,
        functional.configs.stylistic,
        eslintReact.configs["strict-type-checked"],
        reactHooks.configs.flat["recommended-latest"],
      ],
      languageOptions: {
        parserOptions: {
          projectService: true,
          tsconfigRootDir: join(import.meta.dirname, ".."),
        },
      },
      plugins: {
        boundaries,
        guard,
        local,
        "@eslint-community/eslint-comments": eslintComments,
      },
      settings: boundarySettings(
        boundaryTables,
        join(import.meta.dirname, ".."),
      ),
      rules: {
        ...boundaryRules(boundaryTables),
        "local/no-comments": "error",
        "local/no-money-coercion": [
          "error",
          { moneyFile: MONEY_FILE, keyReaders: MONEY_KEY_READERS },
        ],
        "local/forbidden-file": [
          "error",
          {
            checkExtension: false,
            placeholders: FILE_PLACEHOLDERS,
            folders: FILE_FOLDER_ALLOWLISTS,
            featureEntries: boundaryTables.FEATURE_ENTRIES,
          },
        ],
        "local/no-raw-types": "error",
        "local/no-brand-creation": ["error", { mode: "forbid" }],
        "local/no-ui-text": ["error", { screen: "off" }],
        "local/no-promise-catch": "error",
        "local/no-computed-member": "error",
        "local/test-file-boundary": "error",
        "local/ipc-testing-boundary": "error",
        "guard/disable-directive-form": "error",
        "@eslint-community/eslint-comments/no-use": [
          "error",
          { allow: ["eslint-disable-next-line"] },
        ],
        "@eslint-community/eslint-comments/require-description": [
          "error",
          { ignore: [] },
        ],
        "@eslint-community/eslint-comments/no-unlimited-disable": "error",
        "@eslint-community/eslint-comments/no-restricted-disable": [
          "error",
          ...UNDISABLEABLE_RULES,
        ],
        "@eslint-community/eslint-comments/no-duplicate-disable": "error",
        "@typescript-eslint/ban-ts-comment": [
          "error",
          {
            "ts-check": true,
            "ts-expect-error": true,
            "ts-ignore": true,
            "ts-nocheck": true,
          },
        ],

        "@typescript-eslint/no-explicit-any": [
          "error",
          { fixToUnknown: false, ignoreRestArgs: false },
        ],
        "@typescript-eslint/no-unsafe-argument": "error",
        "@typescript-eslint/no-unsafe-assignment": "error",
        "@typescript-eslint/no-unsafe-call": "error",
        "@typescript-eslint/no-unsafe-declaration-merging": "error",
        "@typescript-eslint/no-unsafe-enum-assignment": "error",
        "@typescript-eslint/no-unsafe-enum-comparison": "error",
        "@typescript-eslint/no-unsafe-function-type": "error",
        "@typescript-eslint/no-unsafe-member-access": "error",
        "@typescript-eslint/no-unsafe-return": "error",
        "@typescript-eslint/no-unsafe-type-assertion": "error",
        "@typescript-eslint/no-unsafe-unary-minus": "error",
        "@typescript-eslint/consistent-type-assertions": [
          "error",
          { assertionStyle: "never" },
        ],
        "@typescript-eslint/no-non-null-assertion": "error",
        "@typescript-eslint/strict-boolean-expressions": [
          "error",
          {
            allowString: false,
            allowNumber: false,
            allowNullableObject: false,
            allowNullableBoolean: false,
            allowNullableString: false,
            allowNullableNumber: false,
            allowNullableEnum: false,
            allowAny: false,
          },
        ],
        "@typescript-eslint/explicit-function-return-type": [
          "error",
          {
            allowExpressions: false,
            allowTypedFunctionExpressions: true,
            allowHigherOrderFunctions: false,
            allowDirectConstAssertionInArrowFunctions: false,
            allowConciseArrowFunctionExpressionsStartingWithVoid: false,
          },
        ],
        "@typescript-eslint/explicit-module-boundary-types": "error",
        "@typescript-eslint/method-signature-style": ["error", "property"],

        "functional/no-throw-statements": "error",
        "functional/no-try-statements": "error",
        "functional/no-promise-reject": "error",
        "functional/no-expression-statements": [
          "error",
          { ignoreVoid: true, ignoreSelfReturning: false },
        ],
        "@typescript-eslint/no-floating-promises": [
          "error",
          { ignoreVoid: false, ignoreIIFE: false, checkThenables: true },
        ],
        "@typescript-eslint/no-misused-promises": "error",
        "no-void": "error",
        "@typescript-eslint/no-unused-vars": [
          "error",
          {
            args: "all",
            caughtErrors: "all",
            ignoreRestSiblings: false,
            reportUsedIgnorePattern: true,
          },
        ],

        "functional/no-let": "error",
        "no-var": "error",
        "prefer-const": "error",
        "functional/immutable-data": [
          "error",
          { ignoreClasses: false, ignoreImmediateMutation: false },
        ],
        "no-param-reassign": ["error", { props: true }],
        "no-plusplus": "error",
        "functional/type-declaration-immutability": [
          "error",
          {
            rules: [
              {
                identifiers: "^.+$",
                immutability: "ReadonlyShallow",
                comparator: "AtLeast",
              },
            ],
          },
        ],
        "functional/prefer-immutable-types": [
          "error",
          {
            enforcement: "None",
            ignoreInferredTypes: true,
            parameters: { enforcement: "ReadonlyShallow" },
            returnTypes: { enforcement: "None" },
            variables: { enforcement: "None" },
          },
        ],
        "functional/no-classes": "error",
        "functional/no-class-inheritance": "error",
        "functional/no-this-expressions": "error",
        "functional/no-loop-statements": "error",
        "functional/no-conditional-statements": "off",
        "functional/no-mixed-types": "off",
        "functional/no-return-void": "off",
        "functional/functional-parameters": [
          "error",
          { enforceParameterCount: false },
        ],
        "functional/readonly-type": ["error", "keyword"],
        "no-nested-ternary": "error",
        "no-eval": "error",
        "no-new-func": "error",
        complexity: ["error", 8],
        "max-depth": ["error", 3],
        "max-params": ["error", 3],
        "max-lines": [
          "error",
          { max: 250, skipBlankLines: false, skipComments: false },
        ],
        "max-lines-per-function": [
          "error",
          {
            max: 60,
            skipBlankLines: false,
            skipComments: false,
            IIFEs: true,
          },
        ],
        "no-console": "error",
        "no-alert": "error",
        "no-debugger": "error",
        "no-labels": "error",
        "no-sequences": "error",
        eqeqeq: ["error", "always"],
        "@typescript-eslint/consistent-type-definitions": ["error", "type"],
        "@typescript-eslint/consistent-type-imports": "error",
        "@typescript-eslint/consistent-type-exports": "error",
        "@typescript-eslint/no-import-type-side-effects": "error",
        "@typescript-eslint/prefer-readonly": "error",
        "@typescript-eslint/strict-void-return": "error",

        "@typescript-eslint/switch-exhaustiveness-check": [
          "error",
          {
            allowDefaultCaseForExhaustiveSwitch: false,
            considerDefaultExhaustiveForUnions: false,
            requireDefaultForNonUnion: false,
          },
        ],

        "@typescript-eslint/restrict-plus-operands": [
          "error",
          {
            allowAny: false,
            allowBoolean: false,
            allowNullish: false,
            allowNumberAndString: false,
            allowRegExp: false,
            skipCompoundAssignments: false,
          },
        ],
        "@typescript-eslint/no-base-to-string": "error",

        "react-hooks/rules-of-hooks": "error",
        "react-hooks/exhaustive-deps": "error",
        "@eslint-react/no-array-index-key": "error",
        ...Object.fromEntries(
          Object.entries(banRules).filter(
            ([rule]) => rule !== "guard/forbidden-extension",
          ),
        ),
      },
    },
    {
      files: [
        "src/shared/lib/**",
        "src/shared/ipc/**",
        "src/shared/platform/**",
      ],
      rules: off(["guard/no-number-conversion"]),
    },

    {
      files: SCREEN,
      rules: { "local/no-ui-text": ["error", { screen: "on" }] },
    },
    ...whenListed(CHART_NUMBER_ALLOWLIST, {
      rules: off(["guard/no-number-conversion"]),
    }),
    {
      files: IPC_TESTING,
      rules: off(["guard/no-tauri-mocks", "guard/no-module-mocks"]),
    },
    {
      files: SHARED,
      rules: off(["guard/no-jsx-spread"]),
    },
    ...whenListed(PLATFORM_FILE_ALLOWLIST, {
      rules: off([
        "guard/no-effect-hooks",
        "guard/no-sync-external-store",
        "functional/no-expression-statements",
      ]),
    }),
    ...whenListed(TYPE_PREDICATE_ALLOWLIST, {
      rules: off(["guard/no-type-predicate"]),
    }),
    ...whenListed(THROW_ALLOWLIST, {
      rules: off([
        "functional/no-throw-statements",
        "@typescript-eslint/only-throw-error",
      ]),
    }),
    ...whenListed(PROMISE_REJECTION_ALLOWLIST, {
      rules: off(["local/no-promise-catch"]),
    }),
    ...whenListed(RAW_TYPE_ALLOWLIST, {
      rules: {
        ...off(["local/no-raw-types", ...assertionRules]),
        "local/no-brand-creation": ["error", { mode: "brand-file" }],
      },
    }),
    ...whenListed(MESSAGES_FILES, {
      rules: {
        ...off(["local/no-ui-text", ...assertionRules]),
        "local/no-brand-creation": ["error", { mode: "messages" }],
      },
    }),
    {
      files: FORBIDDEN_FILES,
      linterOptions: { noInlineConfig: true },
      languageOptions: { parser: tseslint.parser },
      plugins: { guard },
      rules: {
        "guard/forbidden-extension": banRules["guard/forbidden-extension"],
      },
    },
    {
      files: OTHER_FILES,
      ignores: [...TS_FILES, ...FORBIDDEN_FILES],
      linterOptions: { noInlineConfig: true },
      languageOptions: { parser: emptyParser },
      plugins: { local },
      rules: {
        "local/forbidden-file": [
          "error",
          {
            checkExtension: true,
            styles: ["src/shared/ds/tokens.css", "src/shared/ds/theme.css"],
            placeholders: FILE_PLACEHOLDERS,
            folders: FILE_FOLDER_ALLOWLISTS,
            featureEntries: boundaryTables.FEATURE_ENTRIES,
          },
        ],
      },
    },
  );
  return warningsAsErrors([
    ...configs,
    ...dsConfigs(),
    ...generatedConfigs(configs, join(import.meta.dirname, "..")),
  ]);
};
