import { brandNamesOf, typeFlags as F } from "../types.mjs";

const HANGUL = /\p{Script=Hangul}/u;
const TEXT_BRANDS = ["UiText"];
const STRING_LIKE =
  F.String | F.StringLiteral | F.TemplateLiteral | F.StringMapping;
const JSX_PARENTS = new Set(["JSXElement", "JSXFragment"]);

const isTextExpression = (expression) =>
  (expression.type === "Literal" && typeof expression.value === "string") ||
  expression.type === "TemplateLiteral";

const unbrandedText = (checker, type, depth = 0) => {
  if (depth > 6) return false;
  const next = (inner) => unbrandedText(checker, inner, depth + 1);
  if (type.isUnion()) return type.types.some(next);
  if (type.isIntersection())
    return (
      !brandNamesOf(checker, type).some((name) => TEXT_BRANDS.includes(name)) &&
      type.types.some(next)
    );
  if ((type.flags & STRING_LIKE) !== 0) return true;
  return checker.isArrayType(type) || checker.isTupleType(type)
    ? checker.getTypeArguments(type).some(next)
    : false;
};

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
        if (screen !== "on" || !JSX_PARENTS.has(node.parent.type)) return;
        if (node.expression.type === "JSXEmptyExpression") return;
        const services = context.sourceCode.parserServices;
        const typed =
          services?.program !== undefined && services.program !== null;
        if (
          isTextExpression(node.expression) ||
          (typed &&
            unbrandedText(
              services.program.getTypeChecker(),
              services.getTypeAtLocation(node.expression),
            ))
        )
          report(node, "jsxText");
      },
    };
  },
};
