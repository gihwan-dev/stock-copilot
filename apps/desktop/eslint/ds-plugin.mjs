import { publicProps } from "./ds-public-props.mjs";
import { noMessageFormat } from "./ds-message-format.mjs";
import { screenRules } from "./ds-screen.mjs";
import { noDomValues, noDomExports } from "./ds-dom.mjs";
import { boundaryRules, intlOptions } from "./ds-boundaries.mjs";
import { styleLiteral, assembledStyle } from "./ds-style-values.mjs";
const rule = (message, create) => ({
  meta: { type: "problem", schema: [], messages: { forbidden: message } },
  create,
});
const literal = (node) =>
  node?.type === "Literal" && typeof node.value === "string";
const branch = (node) =>
  literal(node) ||
  (node?.type === "ConditionalExpression" &&
    branch(node.consequent) &&
    branch(node.alternate)) ||
  (node?.type === "LogicalExpression" &&
    node.operator === "&&" &&
    branch(node.right));
const classes = (node) =>
  literal(node) ||
  (node?.type === "CallExpression" &&
    node.callee.type === "Identifier" &&
    node.callee.name === "cn" &&
    node.arguments.every(branch));
const name = (node) => node?.name ?? node?.value;
const boundCn = (scope) =>
  scope.variables.some((v) => v.name === "cn") ||
  (scope.upper !== null && boundCn(scope.upper));
const report = (ctx, node) => ctx.report({ node, messageId: "forbidden" });

export const dsPlugin = {
  rules: {
    "public-props": publicProps,
    ...boundaryRules,
    ...screenRules,
    "no-message-format": noMessageFormat,
    "no-dom-values": noDomValues,
    "no-dom-exports": noDomExports,
    "static-classes": rule(
      "className은 문자열 리터럴 또는 정적인 cn 인자만 받는다.",
      (ctx) => ({
        JSXAttribute(node) {
          if (
            name(node.name) === "className" &&
            !classes(
              node.value?.type === "JSXExpressionContainer"
                ? node.value.expression
                : node.value,
            )
          )
            report(ctx, node);
        },
        JSXSpreadAttribute(node) {
          report(ctx, node);
        },
        Property(node) {
          if (name(node.key) === "className" && !classes(node.value))
            report(ctx, node);
        },
        CallExpression(node) {
          if (
            node.callee.type === "Identifier" &&
            ["cn", "clsx", "cva", "cx", "twMerge", "twJoin"].includes(
              node.callee.name,
            ) &&
            !(
              node.callee.name === "cn" &&
              node.arguments.every(branch) &&
              !boundCn(ctx.sourceCode.getScope(node))
            )
          )
            report(ctx, node);
        },
      }),
    ),
    "no-inline-style": rule(
      "인라인 style은 허용 파일 밖에서 금지다.",
      (ctx) => ({
        JSXAttribute(node) {
          if (name(node.name) === "style") report(ctx, node);
        },
        Property(node) {
          if (name(node.key) === "style" && !intlOptions(node))
            report(ctx, node);
        },
      }),
    ),
    "no-style-literals": rule(
      "색 리터럴과 px 문자열은 토큰으로 바꾼다.",
      (ctx) => {
        const check = (node, text) => {
          if (styleLiteral(text)) report(ctx, node);
        };
        return {
          Literal(node) {
            if (
              typeof node.value === "string" &&
              !(
                node.parent?.type === "JSXAttribute" &&
                ["href", "xlinkHref"].includes(node.parent.name.name) &&
                node.value.startsWith("#")
              )
            )
              check(node, node.value);
          },
          TemplateLiteral(node) {
            check(node, assembledStyle(node));
          },
          BinaryExpression(node) {
            if (node.operator === "+") check(node, assembledStyle(node));
          },
        };
      },
    ),
  },
};
