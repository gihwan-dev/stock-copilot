import { readList } from "../../../harness/generated-files/generated.mjs";

const desktopPrefix = "apps/desktop/";

export const GENERATED_GLOBALS = [
  "fetch",
  "XMLHttpRequest",
  "WebSocket",
  "EventSource",
  "Worker",
  "SharedWorker",
  "importScripts",
  "navigator",
  "document",
  "location",
  "history",
  "window",
  "globalThis",
  "self",
  "top",
  "parent",
  "frames",
  "opener",
  "localStorage",
  "sessionStorage",
  "indexedDB",
  "caches",
  "cookieStore",
  "eval",
  "Function",
  "Reflect",
  "Proxy",
  "setTimeout",
  "setInterval",
  "queueMicrotask",
  "requestAnimationFrame",
  "postMessage",
  "open",
  "alert",
];

const generatedSyntax = (imports) => [
  {
    selector: "ImportExpression",
    message: "생성물: 동적 import()를 쓰지 않는다.",
  },
  {
    selector:
      "Program > :not(ImportDeclaration, ExportNamedDeclaration[source=null], VariableDeclaration, FunctionDeclaration, TSTypeAliasDeclaration, TSInterfaceDeclaration)",
    message: "생성물: 최상위에는 가져오기와 선언만 둔다.",
  },
  {
    selector:
      ":matches(Program, Program > ExportNamedDeclaration) > VariableDeclaration > VariableDeclarator > :matches(CallExpression, NewExpression, AwaitExpression, TaggedTemplateExpression, SequenceExpression, AssignmentExpression)",
    message: "생성물: 최상위 선언의 초기값에서 코드를 실행하지 않는다.",
  },
  {
    selector:
      "CallExpression[callee.type=/^(ArrowFunctionExpression|FunctionExpression)$/]",
    message: "생성물: 즉시 실행 함수를 쓰지 않는다.",
  },
  {
    selector: "ExportAllDeclaration, ExportNamedDeclaration[source!=null]",
    message: "생성물: 다른 모듈을 다시 내보내지 않는다.",
  },
  {
    selector:
      imports.length === 0
        ? "ImportDeclaration"
        : `ImportDeclaration:not(${imports.map((source) => `[source.value="${source}"]`).join(", ")})`,
    message:
      imports.length === 0
        ? "생성물: 다른 모듈을 가져오지 않는다."
        : `생성물: 가져올 수 있는 모듈은 ${imports.join(", ")}뿐이다.`,
  },
];

export const generatedRules = (imports) => ({
  "no-restricted-globals": ["error", ...GENERATED_GLOBALS],
  "no-restricted-syntax": ["error", ...generatedSyntax(imports)],
  "no-eval": "error",
  "no-implied-eval": "error",
  "no-new-func": "error",
  "no-script-url": "error",
  "no-proto": "error",
  "no-extend-native": "error",
  "no-global-assign": "error",
  "no-with": "error",
});

const everyConfiguredRule = (configs) =>
  Object.fromEntries(
    configs.flatMap((config) =>
      Object.keys(config.rules ?? {}).map((rule) => [rule, "off"]),
    ),
  );

const everyPlugin = (configs) =>
  Object.assign({}, ...configs.map((config) => config.plugins ?? {}));

export const generatedTypeScript = (list = readList()) =>
  list.files
    .filter(
      (entry) =>
        entry.path.startsWith(desktopPrefix) && entry.path.endsWith(".ts"),
    )
    .map((entry) => ({
      path: entry.path.slice(desktopPrefix.length),
      imports: entry.imports,
    }));

export const generatedConfigs = (configs, desktopRoot, list = readList()) =>
  generatedTypeScript(list).map(({ path, imports }) => ({
    name: `generated:${path}`,
    basePath: desktopRoot,
    files: [path],
    plugins: everyPlugin(configs),
    linterOptions: {
      noInlineConfig: true,
      reportUnusedDisableDirectives: "error",
      reportUnusedInlineConfigs: "error",
    },
    rules: { ...everyConfiguredRule(configs), ...generatedRules(imports) },
  }));
