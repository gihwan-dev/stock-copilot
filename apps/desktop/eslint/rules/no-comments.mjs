import { isAllowedDirective } from "../guard-plugin.mjs";

export const noComments = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Disallow every comment except the `// eslint-disable-next-line <one rule> -- <reason>` form",
    },
    schema: [],
    messages: {
      comment:
        "주석 금지: 설명은 이름, 타입, 테스트, docs/에 쓴다. 허용하는 것은 `// eslint-disable-next-line <규칙 하나> -- <이유>` 한 줄뿐이다.",
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
        .getAllComments()
        .filter((comment) => !allowed.has(comment))
        .forEach((comment) =>
          context.report({ loc: comment.loc, messageId: "comment" }),
        );
    },
  }),
};
