import js from "@eslint/js";
import eslintComments from "@eslint-community/eslint-plugin-eslint-comments";
import eslintReact from "@eslint-react/eslint-plugin";
import functional from "eslint-plugin-functional";
import reactHooks from "eslint-plugin-react-hooks";
import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";
import { guard } from "./eslint/guard-plugin.mjs";

const TS_FILES = ["src/**/*.ts", "src/**/*.tsx"];
const FEATURES = ["src/features/**"];
const DS = ["src/shared/ds/**"];
const IPC_TESTING = ["src/shared/ipc/testing/**"];
const FORBIDDEN_FILES = [
  "src/**/*.js",
  "src/**/*.jsx",
  "src/**/*.mjs",
  "src/**/*.cjs",
  "src/**/*.mts",
  "src/**/*.cts",
  "src/**/*.d.ts",
];

const PLATFORM_FILE_ALLOWLIST = [
  "src/shared/platform/index.ts",
  "src/shared/platform/detach.ts",
  "src/shared/platform/useCoreEvent.ts",
  "src/shared/platform/useDocumentTitle.ts",
];
const TYPE_PREDICATE_ALLOWLIST = ["src/shared/lib/money.ts"];
const THROW_ALLOWLIST = [];
const CHART_NUMBER_ALLOWLIST = [];

const UNDISABLEABLE_RULES = [
  "local/no-comments",
  "local/no-money-coercion",
  "local/forbidden-file",
  "boundaries/*",
  "guard/*",
  "@typescript-eslint/ban-ts-comment",
  "@eslint-community/eslint-comments/*",
];

const effectHooks = ["useEffect", "useLayoutEffect", "useInsertionEffect"];
const effectPattern = "/^use(Layout|Insertion)?Effect$/";
const networkPattern =
  "/\\b(fetch|XMLHttpRequest|WebSocket|EventSource|sendBeacon|defaultView)\\b/";
const skipNames = "/^(skip|only|todo|skipIf|runIf)$/";

const syntax = {
  enum: {
    selector: "TSEnumDeclaration",
    message: "enum 금지. 문자열 리터럴 유니온을 쓴다.",
  },
  namespace: {
    selector: "TSModuleDeclaration",
    message: "namespace·declare module·declare global 금지.",
  },
  declare: {
    selector: "[declare=true]",
    message: "declare 금지. 존재하지 않는 값을 타입으로 꾸며낼 수 있다.",
  },
  overload: {
    selector: "TSDeclareFunction",
    message: "함수 오버로드 금지. 구현 시그니처가 거짓말을 할 수 있다.",
  },
  typePredicate: {
    selector: "TSTypePredicate",
    message:
      "타입 술어(x is T)와 단언 함수(asserts x) 금지. 스키마 파싱을 쓴다.",
  },
  defaultCase: {
    selector: "SwitchCase[test=null]",
    message: "switch의 default 팔 금지. 모든 경우를 나열한다.",
  },
  defaultExport: {
    selector: "ExportDefaultDeclaration",
    message: "기본 내보내기 금지.",
  },
  exportAll: {
    selector: "ExportAllDeclaration",
    message: "export * 금지. 내보낼 이름을 나열한다.",
  },
  dynamicImport: {
    selector: "ImportExpression",
    message: "동적 import 금지.",
  },
  unaryPlus: {
    selector: "UnaryExpression[operator='+']",
    message: "단항 + 변환 금지.",
  },
  throwingParse: {
    selector: "CallExpression[callee.property.name=/^(parse|parseAsync)$/]",
    message: "예외를 던지는 parse 금지. safeParse로 Result를 만든다.",
  },
  reactNamespace: {
    selector:
      "ImportDeclaration[source.value='react'] > :matches(ImportNamespaceSpecifier, ImportDefaultSpecifier)",
    message: "react는 이름 있는 가져오기만 쓴다.",
  },
  prototype: {
    selector:
      ":matches(MemberExpression[property.name=/^(prototype|__proto__)$/], MemberExpression[property.value=/^(prototype|__proto__)$/])",
    message: "prototype 접근 금지.",
  },
  callApply: {
    selector:
      ":matches(MemberExpression[property.name=/^(call|apply|bind)$/], MemberExpression[property.value=/^(call|apply|bind)$/])",
    message: "call·apply·bind 금지.",
  },
  effectName: {
    selector: `:matches(Identifier[name=${effectPattern}], Literal[value=${effectPattern}])`,
    message: "useEffect 계열은 shared/platform의 허용 목록 파일에서만.",
  },
  numericCoercion: {
    selector:
      ":matches(CallExpression, NewExpression)[callee.name=/^(Number|parseFloat|parseInt|BigInt)$/]",
    message:
      "Number·parseFloat·parseInt·BigInt 호출 금지. 코어가 보낸 표시용 값을 쓴다.",
  },
  numericMember: {
    selector: "MemberExpression[object.name=/^(Number|BigInt|Math)$/]",
    message: "Number·BigInt·Math 멤버 사용 금지. 계산은 코어가 한다.",
  },
  refFunction: {
    selector:
      "JSXAttribute[name.name='ref'] > JSXExpressionContainer > :matches(ArrowFunctionExpression, FunctionExpression)",
    message: "ref 콜백 함수 금지. 마운트 효과 우회다.",
  },
};

const baseSyntax = [
  syntax.enum,
  syntax.namespace,
  syntax.declare,
  syntax.overload,
  syntax.typePredicate,
  syntax.defaultCase,
  syntax.defaultExport,
  syntax.exportAll,
  syntax.dynamicImport,
  syntax.unaryPlus,
  syntax.throwingParse,
  syntax.reactNamespace,
  syntax.prototype,
  syntax.callApply,
];
const numberSyntax = [syntax.numericCoercion, syntax.numericMember];

const restrictedSyntax = ({ effects, predicates, numbers, refFunctions }) => [
  "error",
  ...baseSyntax.filter(
    (entry) => predicates === "deny" || entry !== syntax.typePredicate,
  ),
  ...(effects === "deny" ? [syntax.effectName] : []),
  ...(numbers === "deny" ? numberSyntax : []),
  ...(refFunctions === "deny" ? [syntax.refFunction] : []),
];

const restrictedImports = ({ effects, tauriMocks }) => [
  "error",
  {
    paths: [
      {
        name: "react",
        importNames: [
          "default",
          ...(effects === "deny"
            ? [...effectHooks, "useSyncExternalStore"]
            : []),
        ],
        message:
          "react 기본 가져오기 금지. useEffect 계열과 useSyncExternalStore는 shared/platform의 허용 목록 파일에서만.",
      },
    ],
    patterns: [
      {
        group: ["react/*", "react-dom/*", "!react-dom/client"],
        message: "react 하위 경로 가져오기 금지.",
      },
      ...(tauriMocks === "deny"
        ? [
            {
              group: ["@tauri-apps/api/mocks", "@tauri-apps/api/mocks/*"],
              message:
                "@tauri-apps/api/mocks는 shared/ipc/testing에서만 가져온다.",
            },
          ]
        : []),
    ],
  },
];

const defaults = {
  effects: "deny",
  predicates: "deny",
  numbers: "allow",
  refFunctions: "allow",
  tauriMocks: "deny",
};

const override = (files, changes, extraRules) => {
  const settings = { ...defaults, ...changes };
  return {
    files,
    rules: {
      "no-restricted-syntax": restrictedSyntax(settings),
      "no-restricted-imports": restrictedImports(settings),
      ...extraRules,
    },
  };
};

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

export default warningsAsErrors(
  defineConfig(
    {
      ignores: ["dist/**", "src-tauri/**", "e2e-app/**", "node_modules/**"],
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
          tsconfigRootDir: import.meta.dirname,
        },
      },
      plugins: {
        guard,
        "@eslint-community/eslint-comments": eslintComments,
      },
      rules: {
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
        "no-restricted-properties": [
          "error",
          { object: "Object", property: "assign" },
          { object: "Object", property: "defineProperty" },
          { object: "Object", property: "defineProperties" },
          { object: "Object", property: "setPrototypeOf" },
          { object: "Object", property: "getOwnPropertySymbols" },
          { object: "Object", property: "getOwnPropertyDescriptor" },
          { object: "Object", property: "getOwnPropertyDescriptors" },
          { object: "Reflect" },
          { object: "Proxy" },
          { object: "JSON", property: "parse" },
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
        "no-restricted-globals": [
          "error",
          "event",
          "name",
          "length",
          "status",
          "globalThis",
          "window",
          "self",
          "Reflect",
          "Proxy",
        ],
        complexity: ["error", 8],
        "max-depth": ["error", 3],
        "max-params": ["error", 3],
        "max-lines": [
          "error",
          { max: 250, skipBlankLines: false, skipComments: false },
        ],
        "max-lines-per-function": [
          "error",
          { max: 60, skipBlankLines: false, skipComments: false, IIFEs: true },
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

        "guard/no-global-object": [
          "error",
          "globalThis",
          "window",
          "self",
          "top",
          "parent",
          "frames",
          "opener",
          "eval",
          "Function",
        ],
        "guard/no-network": [
          "error",
          {
            selector: `:matches(Identifier[name=${networkPattern}], Literal[value=${networkPattern}], TemplateElement[value.raw=${networkPattern}])`,
            message:
              "fetch·XMLHttpRequest·WebSocket 등 네트워크 사용 금지. UI는 IPC로만 통신한다.",
          },
        ],
        "guard/no-test-skip": [
          "error",
          {
            selector: `:matches(MemberExpression[property.name=${skipNames}], MemberExpression[property.value=${skipNames}])`,
            message: "테스트 건너뛰기·집중 금지.",
          },
          {
            selector:
              "MemberExpression[object.name=/^(describe|it|test|suite|bench)$/][property.name=/^(fails|concurrent)$/]",
            message: "테스트 실패 뒤집기·동시 실행 표시 금지.",
          },
          {
            selector:
              "CallExpression[callee.name=/^(xit|xtest|xdescribe|fit|fdescribe)$/]",
            message: "테스트 건너뛰기·집중 금지.",
          },
        ],
        "no-restricted-syntax": restrictedSyntax(defaults),
        "no-restricted-imports": restrictedImports(defaults),
      },
    },
    override(FEATURES, { numbers: "deny", refFunctions: "deny" }, {}),
    override(DS, { numbers: "deny" }, {}),
    ...whenListed(
      CHART_NUMBER_ALLOWLIST,
      override([], { numbers: "allow" }, {}),
    ),
    override(IPC_TESTING, { tauriMocks: "allow" }, {}),
    ...whenListed(
      PLATFORM_FILE_ALLOWLIST,
      override(
        [],
        { effects: "allow", refFunctions: "allow" },
        { "functional/no-expression-statements": "off" },
      ),
    ),
    ...whenListed(
      TYPE_PREDICATE_ALLOWLIST,
      override([], { predicates: "allow" }, {}),
    ),
    ...whenListed(THROW_ALLOWLIST, {
      rules: { "functional/no-throw-statements": "off" },
    }),
    {
      files: FORBIDDEN_FILES,
      linterOptions: { noInlineConfig: true },
      languageOptions: { parser: tseslint.parser },
      plugins: { guard },
      rules: {
        "guard/forbidden-extension": [
          "error",
          {
            selector: "Program",
            message:
              "src에는 .ts·.tsx 파일만 둔다. .js·.jsx·.mjs·.cjs·.mts·.cts·.d.ts는 금지다.",
          },
        ],
      },
    },
  ),
);
