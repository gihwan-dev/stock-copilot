import { propertyName } from "./ds-boundaries.mjs";
import { DS_CLASSES } from "./ds-classes.mjs";
import { brandNamesOf, typeFlags } from "./types.mjs";

const rule = (message, create) => ({
  meta: { type: "problem", schema: [], messages: { forbidden: message } },
  create,
});
const report = (ctx, node) => ctx.report({ node, messageId: "forbidden" });
const classes = new Set(DS_CLASSES);
const utility = (value) =>
  classes.has(value) ||
  /^(?:-?(?:bg|text|border|p[xytrblse]?|m[xytrblse]?|gap|w|h|min-w|max-w|min-h|max-h|fill|stroke|rounded|shadow|grid-cols|z|opacity|overflow|font|leading|tracking|duration|translate-[xy])-)(?:\d+(?:\/\d+)?|px|auto|full|screen|min|max|fit|none)$/u.test(
    value,
  );
const classLike = (text) => {
  const values = text.trim().split(/\s+/u);
  return (
    values.some(
      (value) =>
        /(?:\[[^\]]+\]|\(--[\w-]+\))/u.test(value) ||
        /^(?:hover|focus|focus-visible|focus-within|active|disabled|sm|md|lg|xl|2xl|dark|motion-reduce|window-md|window-lg|@content-medium|@content-wide):/u.test(
          value,
        ),
    ) ||
    (values.length > 1 && values.every(utility))
  );
};
const helpers = [
  "cn",
  "clsx",
  "cva",
  "cx",
  "twMerge",
  "twJoin",
  "css",
  "styled",
];
const forbiddenImport = (source) =>
  /\.(?:css|scss|sass|less|styl)(?:[?].*)?$/iu.test(source);
const textProps = [
  "aria-label",
  "aria-description",
  "aria-valuetext",
  "aria-roledescription",
  "aria-placeholder",
  "aria-braillelabel",
  "aria-brailleroledescription",
  "aria-colindextext",
  "aria-rowindextext",
  "title",
  "alt",
  "placeholder",
  "label",
  "description",
  "summary",
  "abbr",
  "prompt",
  "standby",
];
const textAttribute = (node) =>
  textProps.includes(propertyName(node.name)) ||
  (propertyName(node.name) === "value" &&
    propertyName(node.parent.name) === "input" &&
    node.parent.attributes.some(
      (attribute) =>
        attribute.type === "JSXAttribute" &&
        propertyName(attribute.name) === "type" &&
        ["button", "submit", "reset"].includes(attribute.value?.value),
    ));
const intrinsicType = (type) =>
  type?.isUnion()
    ? type.types.some(intrinsicType)
    : (type?.flags & typeFlags.StringLike) !== 0;
const uiText = (checker, type) =>
  type.isUnion()
    ? type.types.every((part) => uiText(checker, part))
    : brandNamesOf(checker, type).includes("UiText");
export const screenRules = {
  "no-screen-elements": rule("화면에서는 DS 컴포넌트만 조립한다.", (ctx) => ({
    JSXOpeningElement(node) {
      const name = node.name;
      const services = ctx.sourceCode.parserServices;
      if (
        ["JSXNamespacedName", "JSXMemberExpression"].includes(name.type) ||
        (name.type === "JSXIdentifier" && /^[a-z]/u.test(name.name)) ||
        (typeof services.getTypeAtLocation === "function" &&
          intrinsicType(services.getTypeAtLocation(name)))
      )
        report(ctx, node);
    },
  })),
  "no-screen-props": rule(
    "화면 코드에서 className·class·style 속성은 금지다.",
    (ctx) => ({
      JSXAttribute(node) {
        if (["className", "class", "style"].includes(propertyName(node.name)))
          report(ctx, node);
      },
    }),
  ),
  "no-screen-imports": rule(
    "스타일 파일·도우미·UI 라이브러리는 DS 안에서만 가져온다.",
    (ctx) => ({
      ImportDeclaration(node) {
        if (forbiddenImport(node.source.value)) report(ctx, node);
      },
      ExportNamedDeclaration(node) {
        if (node.source !== null && forbiddenImport(node.source.value))
          report(ctx, node);
      },
      ExportAllDeclaration(node) {
        if (forbiddenImport(node.source.value)) report(ctx, node);
      },
      Identifier(node) {
        if (helpers.includes(node.name)) report(ctx, node);
      },
    }),
  ),
  "no-screen-classes": rule(
    "화면 코드에서 클래스 문자열을 만들거나 넘기지 않는다.",
    (ctx) => ({
      Literal(node) {
        if (
          typeof node.value === "string" &&
          classLike(node.value) &&
          node.parent.type !== "ImportDeclaration"
        )
          report(ctx, node);
      },
      TemplateElement(node) {
        if (classLike(node.value.cooked ?? "")) report(ctx, node);
      },
    }),
  ),
  "no-screen-format": rule(
    "형식 변환은 shared/lib/format에서, 화면 문구는 messages.ts에서 받는다.",
    (ctx) => {
      return {
        Identifier(node) {
          if (node.name === "Intl") report(ctx, node);
        },
        MemberExpression(node) {
          if (
            [
              "toLocaleString",
              "toLocaleDateString",
              "toLocaleTimeString",
              "toFixed",
              "toExponential",
              "toPrecision",
            ].includes(propertyName(node.property))
          )
            report(ctx, node);
        },
        Property(node) {
          if (
            [
              "toLocaleString",
              "toLocaleDateString",
              "toLocaleTimeString",
              "toFixed",
              "toExponential",
              "toPrecision",
            ].includes(propertyName(node.key))
          )
            report(ctx, node);
        },
      };
    },
  ),
  "no-ui-attributes": rule(
    "JSX 사용자 문구 속성에는 messages.ts의 UiText만 넘긴다.",
    (ctx) => ({
      JSXAttribute(node) {
        if (!textAttribute(node)) return;
        const value = node.value;
        const services = ctx.sourceCode.parserServices;
        if (
          value?.type !== "JSXExpressionContainer" ||
          typeof services.getTypeAtLocation !== "function" ||
          !uiText(
            services.program.getTypeChecker(),
            services.getTypeAtLocation(value.expression),
          )
        )
          report(ctx, node);
      },
    }),
  ),
};
