const member = (name) =>
  `:matches(MemberExpression[property.name='${name}'], MemberExpression[property.value='${name}'], MemberExpression[property.quasis.0.value.raw='${name}'])`;
const destructured = (name) =>
  `:matches(ObjectPattern > Property[key.name='${name}'], ObjectPattern > Property[key.value='${name}'])`;
const testObjects = "/^(describe|it|test|suite|bench)$/";
const testOption = (name) =>
  `:matches(CallExpression[callee.name=${testObjects}], CallExpression[callee.object.name=${testObjects}]) > ObjectExpression > Property[key.name='${name}']`;
const word = (name) =>
  `:matches(Identifier[name='${name}'], Literal[value=/\\b${name}\\b/], TemplateElement[value.raw=/\\b${name}\\b/])`;
const exactName = (name) =>
  `:matches(Identifier[name='${name}'], Literal[value='${name}'], TemplateElement[value.raw='${name}'])`;
const importedName = (name) =>
  `:matches(ImportSpecifier[imported.name='${name}'], ExportSpecifier[local.name='${name}'], MemberExpression[property.name='${name}'], MemberExpression[property.value='${name}'])`;

const globalsBan = (names, reason) =>
  names.map((name) => ({ name, message: `${name} 금지: ${reason}` }));

export const bans = {
  "guard/no-enum": [
    {
      selector: "TSEnumDeclaration",
      message: "enum 금지: 리터럴 유니온을 쓴다.",
    },
  ],
  "guard/no-namespace": [
    {
      selector: "TSModuleDeclaration",
      message: "namespace·declare module·declare global 금지.",
    },
  ],
  "guard/no-declare": [
    {
      selector: "[declare=true]",
      message: "declare 금지: 없는 값을 타입으로 꾸며낼 수 있다.",
    },
  ],
  "guard/no-overload": [
    {
      selector: "TSDeclareFunction",
      message: "함수 오버로드 금지: 구현 시그니처가 거짓말을 할 수 있다.",
    },
  ],
  "guard/no-type-predicate": [
    {
      selector: "TSTypePredicate",
      message:
        "타입 술어(x is T)와 단언 함수(asserts x) 금지: 스키마 파싱을 쓴다.",
    },
  ],
  "guard/no-default-case": [
    {
      selector: "SwitchCase[test=null]",
      message: "switch의 default 팔 금지: 모든 경우를 나열한다.",
    },
  ],
  "guard/no-default-export": [
    { selector: "ExportDefaultDeclaration", message: "기본 내보내기 금지." },
  ],
  "guard/no-export-all": [
    {
      selector: "ExportAllDeclaration",
      message: "export * 금지: 내보낼 이름을 나열한다.",
    },
  ],
  "guard/no-dynamic-import": [
    { selector: "ImportExpression", message: "동적 import 금지." },
  ],
  "guard/no-unary-plus": [
    {
      selector: "UnaryExpression[operator='+']",
      message: "단항 + 숫자 변환 금지.",
    },
  ],
  "guard/no-throwing-parse": [
    {
      selector: `:matches(${member("parse")}, ${destructured("parse")})`,
      message: "parse 금지: 던지는 parse 대신 safeParse로 Result를 만든다.",
    },
    {
      selector: `:matches(${member("parseAsync")}, ${destructured("parseAsync")})`,
      message:
        "parseAsync 금지: 던지는 parseAsync 대신 safeParseAsync로 Result를 만든다.",
    },
  ],
  "guard/no-react-namespace": [
    {
      selector:
        "ImportDeclaration[source.value='react'] > :matches(ImportNamespaceSpecifier, ImportDefaultSpecifier)",
      message:
        "react 기본·네임스페이스 가져오기 금지: 이름 있는 가져오기만 쓴다.",
    },
    {
      selector:
        "ImportDeclaration[source.value='react'] > ImportSpecifier[imported.name='default']",
      message: "react의 default를 이름으로 가져오기 금지.",
    },
  ],
  "guard/no-prototype": ["prototype", "__proto__"].map((name) => ({
    selector: `:matches(${member(name)}, ${destructured(name)})`,
    message: `${name} 접근 금지: 불투명 값의 내부와 내장 변경 함수에 닿는 길이다.`,
  })),
  "guard/no-call-apply-bind": ["call", "apply", "bind"].map((name) => ({
    selector: `:matches(${member(name)}, ${destructured(name)})`,
    message: `${name} 금지: 다른 this로 변경 함수를 부르는 길이다.`,
  })),
  "guard/no-effect-hooks": [
    "useEffect",
    "useLayoutEffect",
    "useInsertionEffect",
    "useImperativeHandle",
  ].map((name) => ({
    selector: exactName(name),
    message: `${name} 금지: 효과 훅은 shared/platform의 허용 목록 파일에서만.`,
  })),
  "guard/no-sync-external-store": [
    {
      selector: exactName("useSyncExternalStore"),
      message:
        "useSyncExternalStore 금지: shared/platform의 허용 목록 파일에서만.",
    },
  ],
  "guard/no-ref-callback": [
    {
      selector:
        ":matches(JSXAttribute[name.name='ref'] > JSXExpressionContainer > :not(Identifier), JSXAttribute[name.name='ref'] > Literal)",
      message:
        "ref에는 식별자만 넘긴다: 함수·호출·조건식 ref는 마운트 효과 우회다.",
    },
    {
      selector:
        "JSXSpreadAttribute Property:matches([key.name='ref'], [key.value='ref'])",
      message: "스프레드 속성 안의 ref 금지.",
    },
  ],
  "guard/no-network": [
    ...[
      "fetch",
      "XMLHttpRequest",
      "WebSocket",
      "EventSource",
      "sendBeacon",
      "defaultView",
      "Worker",
      "SharedWorker",
      "RTCPeerConnection",
      "WebTransport",
      "importScripts",
    ].map((name) => ({
      selector: word(name),
      message: `${name} 금지: UI는 IPC로만 통신한다.`,
    })),
    ...[
      "preload",
      "preinit",
      "preconnect",
      "prefetchDNS",
      "preloadModule",
      "preinitModule",
    ].map((name) => ({
      selector: importedName(name),
      message: `react-dom ${name} 금지: 리소스를 미리 받는 네트워크 요청이다.`,
    })),
  ],
  "guard/no-test-skip": [
    ...["skip", "only", "todo", "skipIf", "runIf"].map((name) => ({
      selector: `:matches(${member(name)}, ${destructured(name)}, ${testOption(name)})`,
      message: `테스트 ${name} 금지: 테스트를 건너뛰거나 골라 돌리지 않는다.`,
    })),
    ...["fails", "concurrent"].map((name) => ({
      selector: `:matches(MemberExpression[object.name=${testObjects}][property.name='${name}'], VariableDeclarator[init.name=${testObjects}] > ObjectPattern > Property[key.name='${name}'], ${testOption(name)})`,
      message: `테스트 ${name} 금지: 실패 뒤집기·동시 실행 표시를 쓰지 않는다.`,
    })),
    ...["xit", "xtest", "xdescribe", "fit", "fdescribe"].map((name) => ({
      selector: `Identifier[name='${name}']`,
      message: `${name} 금지: 테스트를 건너뛰거나 골라 돌리지 않는다.`,
    })),
  ],
  "guard/no-promise-catch": [
    {
      selector: `:matches(${member("catch")}, ${destructured("catch")})`,
      message:
        "catch 금지: 거절을 Result로 바꾸는 곳은 shared/ipc 호출 어댑터와 shared/platform/detach뿐이다.",
    },
    {
      selector:
        ":matches(CallExpression[callee.property.name='then'], CallExpression[callee.property.value='then'], CallExpression[callee.property.quasis.0.value.raw='then']):matches([arguments.length>1], :has(> SpreadElement))",
      message:
        "두 인자 then 금지: 거절 처리기는 shared/ipc 호출 어댑터와 shared/platform/detach에서만 붙인다.",
    },
  ],
  "guard/forbidden-extension": [
    {
      selector: "Program",
      message:
        "src에는 .ts·.tsx 파일만 둔다. .js·.jsx·.mjs·.cjs·.mts·.cts·.d.ts는 금지다.",
    },
  ],
  "guard/no-global-object": globalsBan(
    [
      "globalThis",
      "window",
      "self",
      "top",
      "parent",
      "frames",
      "opener",
      "open",
      "eval",
      "Function",
    ],
    "전역 객체와 문자열 실행으로 네트워크 금지를 우회할 수 있다.",
  ),
  "guard/no-dom-globals": globalsBan(
    [
      "document",
      "navigator",
      "location",
      "localStorage",
      "sessionStorage",
      "indexedDB",
    ],
    "브라우저 전역은 shared/ds와 shared/platform 허용 목록 파일에서만.",
  ),
  "guard/no-number-conversion": globalsBan(
    ["Number", "Math", "parseFloat", "parseInt", "BigInt"],
    "화면 코드는 숫자를 바꾸거나 계산하지 않는다. 코어가 보낸 표시용 값을 쓴다.",
  ),
  "guard/no-reflect": globalsBan(
    ["Reflect"],
    "불투명 금액의 symbol 키를 꺼내는 길이다.",
  ),
  "guard/no-proxy": globalsBan(["Proxy"], "값 접근을 가로채는 길이다."),
  "guard/no-own-symbols": [
    "getOwnPropertySymbols",
    "getOwnPropertyDescriptor",
    "getOwnPropertyDescriptors",
  ].map((property) => ({
    object: "Object",
    property,
    message: `Object.${property} 금지: 불투명 금액의 symbol 키를 꺼내는 길이다.`,
  })),
  "guard/no-object-mutation": [
    "assign",
    "defineProperty",
    "defineProperties",
    "setPrototypeOf",
  ].map((property) => ({
    object: "Object",
    property,
    message: `Object.${property} 금지: 객체를 바꾸지 않고 새로 만든다.`,
  })),
  "guard/no-json-parse": [
    {
      object: "JSON",
      property: "parse",
      message: "JSON.parse 금지: any를 돌려준다. 경계에서 스키마로 파싱한다.",
    },
  ],
  "no-restricted-globals": globalsBan(
    ["event", "name", "length", "status", "globalThis", "window", "self"],
    "암묵 전역을 쓰지 않는다.",
  ),
};

export const importBans = {
  "guard/no-react-subpath": {
    patterns: [
      {
        group: ["react/*"],
        message: "react 하위 경로 가져오기 금지.",
      },
      {
        group: ["react-dom/*", "!react-dom/client"],
        message: "react-dom 하위 경로 가져오기 금지(react-dom/client만 허용).",
      },
    ],
  },
  "guard/no-tauri-mocks": {
    patterns: [
      {
        group: ["@tauri-apps/api/mocks", "@tauri-apps/api/mocks/*"],
        message: "@tauri-apps/api/mocks는 shared/ipc/testing에서만 가져온다.",
      },
    ],
  },
};
