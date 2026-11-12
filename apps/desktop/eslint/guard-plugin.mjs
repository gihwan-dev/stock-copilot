import { builtinRules } from "eslint/use-at-your-own-risk";

const singleRule = /^\s*[^\s,]+\s*$/u;

export const isAllowedDirective = (directive) =>
  directive.type === "disable-next-line" &&
  directive.node.type === "Line" &&
  singleRule.test(directive.value) &&
  directive.justification.trim().length > 0;

const disableDirectiveForm = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Allow only `// eslint-disable-next-line <one rule> -- <reason>` as inline configuration",
    },
    schema: [],
    messages: {
      form: "검사 끄기는 `// eslint-disable-next-line <규칙 하나> -- <이유>` 형태만 쓴다. 다른 인라인 설정 주석은 금지다.",
    },
  },
  create: (context) => ({
    Program: () => {
      const { sourceCode } = context;
      const allowed = new Set(
        sourceCode
          .getDisableDirectives()
          .directives.filter(isAllowedDirective)
          .map((directive) => directive.node),
      );
      sourceCode
        .getInlineConfigNodes()
        .filter((comment) => !allowed.has(comment))
        .forEach((comment) =>
          context.report({ loc: comment.loc, messageId: "form" }),
        );
    },
  }),
};

export const guardRuleBase = {
  "forbidden-extension": "no-restricted-syntax",
  "no-call-apply-bind": "no-restricted-syntax",
  "no-declare": "no-restricted-syntax",
  "no-default-case": "no-restricted-syntax",
  "no-default-export": "no-restricted-syntax",
  "no-disposable": "no-restricted-syntax",
  "no-dom-globals": "no-restricted-globals",
  "no-dynamic-import": "no-restricted-syntax",
  "no-effect-hooks": "no-restricted-syntax",
  "no-enum": "no-restricted-syntax",
  "no-export-all": "no-restricted-syntax",
  "no-global-object": "no-restricted-globals",
  "no-import-meta": "no-restricted-syntax",
  "no-json-parse": "no-restricted-properties",
  "no-jsx-spread": "no-restricted-syntax",
  "no-module-mocks": "no-restricted-syntax",
  "no-namespace": "no-restricted-syntax",
  "no-network": "no-restricted-syntax",
  "no-number-conversion": "no-restricted-globals",
  "no-object-mutation": "no-restricted-properties",
  "no-overload": "no-restricted-syntax",
  "no-own-symbols": "no-restricted-properties",
  "no-prototype": "no-restricted-syntax",
  "no-namespace-import": "no-restricted-syntax",
  "no-proxy": "no-restricted-globals",
  "no-react-namespace": "no-restricted-syntax",
  "no-react-subpath": "no-restricted-imports",
  "no-ref-callback": "no-restricted-syntax",
  "no-reflect": "no-restricted-globals",
  "no-sync-external-store": "no-restricted-syntax",
  "no-tauri-mocks": "no-restricted-imports",
  "no-test-skip": "no-restricted-syntax",
  "no-throwing-parse": "no-restricted-syntax",
  "no-type-predicate": "no-restricted-syntax",
  "no-unary-plus": "no-restricted-syntax",
};

export const guard = {
  meta: { name: "guard" },
  rules: {
    "disable-directive-form": disableDirectiveForm,
    ...Object.fromEntries(
      Object.entries(guardRuleBase).map(([name, base]) => [
        name,
        builtinRules.get(base),
      ]),
    ),
  },
};
