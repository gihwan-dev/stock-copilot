const HANGUL = /\p{Script=Hangul}/u;
const JSX_PARENTS = new Set(["JSXElement", "JSXFragment"]);

const isTextExpression = (expression) =>
  (expression.type === "Literal" && typeof expression.value === "string") ||
  expression.type === "TemplateLiteral";

export const noUiText = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Keep user-facing text in messages.ts: Hangul literals anywhere else and JSX text in screen code fail",
    },
    schema: [
      {
        type: "object",
        properties: {
          screen: { enum: ["on", "off"] },
        },
        required: ["screen"],
        additionalProperties: false,
      },
    ],
    messages: {
      hangul: "한글 문구는 messages.ts에만 둔다. 화면에는 UiText를 넘긴다.",
      jsxText:
        "화면 코드와 shared/ds에 JSX 글자를 직접 쓰지 않는다. messages.ts의 UiText를 넘긴다.",
    },
  },
  create: (context) => {
    const [{ screen }] = context.options;
    const report = (node, messageId) => context.report({ node, messageId });
    return {
      Literal: (node) => {
        if (typeof node.value === "string" && HANGUL.test(node.value))
          report(node, "hangul");
      },
      TemplateElement: (node) => {
        if (HANGUL.test(node.value.raw)) report(node, "hangul");
      },
      JSXText: (node) => {
        if (HANGUL.test(node.value)) report(node, "hangul");
        else if (screen === "on" && node.value.trim() !== "")
          report(node, "jsxText");
      },
      JSXExpressionContainer: (node) => {
        if (
          screen === "on" &&
          JSX_PARENTS.has(node.parent.type) &&
          isTextExpression(node.expression)
        )
          report(node, "jsxText");
      },
    };
  },
};
