import { builtinRules } from "eslint/use-at-your-own-risk";

const restrictedSyntax = builtinRules.get("no-restricted-syntax");
const restrictedGlobals = builtinRules.get("no-restricted-globals");

const singleRule = /^\s*[^\s,]+\s*$/u;

const isAllowedDirective = (directive) =>
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

export const guard = {
  meta: { name: "guard" },
  rules: {
    "disable-directive-form": disableDirectiveForm,
    "forbidden-extension": restrictedSyntax,
    "no-global-object": restrictedGlobals,
    "no-network": restrictedSyntax,
    "no-test-skip": restrictedSyntax,
  },
};
