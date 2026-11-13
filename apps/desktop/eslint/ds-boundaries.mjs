const rule = (message, create) => ({
  meta: { type: "problem", schema: [], messages: { forbidden: message } },
  create,
});
export const propertyName = (node) => {
  if (node?.type === "Identifier" || node?.type === "JSXIdentifier")
    return node.name;
  if (node?.type === "Literal") return node.value;
  if (node?.type === "TemplateLiteral" && node.expressions.length === 0)
    return node.quasis[0].value.cooked;
  if (node?.type === "BinaryExpression" && node.operator === "+")
    return node.left.type === "Identifier" || node.right.type === "Identifier"
      ? undefined
      : `${propertyName(node.left)}${propertyName(node.right)}`;
  return undefined;
};
const report = (ctx, node) => ctx.report({ node, messageId: "forbidden" });
export const DOM_STYLE_MEMBERS = [
  "classList",
  "className",
  "style",
  "setAttribute",
  "setAttributeNS",
  "toggleAttribute",
  "setProperty",
  "cssText",
  "animate",
  "insertAdjacentHTML",
  "innerHTML",
  "outerHTML",
  "adoptedStyleSheets",
  "insertRule",
  "CSSStyleSheet",
  "attributeStyleMap",
];
export const PRESENTATION_PROPS = [
  "display",
  "visibility",
  "overflow",
  "cursor",
  "pointerEvents",
  "paintOrder",
  "vectorEffect",
  "shapeRendering",
  "textRendering",
  "colorInterpolation",
  "colorRendering",
  "colorInterpolationFilters",
  "floodColor",
  "floodOpacity",
  "lightingColor",
  "stopColor",
  "stopOpacity",
  "fontStyle",
  "fontVariant",
  "fontStretch",
  "wordSpacing",
  "textDecoration",
  "writingMode",
  "direction",
  "unicodeBidi",
  "isolation",
  "mixBlendMode",
  "markerStart",
  "markerMid",
  "markerEnd",
  "fill",
  "stroke",
  "strokeWidth",
  "strokeDasharray",
  "strokeDashoffset",
  "strokeLinecap",
  "strokeLinejoin",
  "strokeMiterlimit",
  "fillRule",
  "clipRule",
  "clipPath",
  "mask",
  "filter",
  "width",
  "height",
  "minWidth",
  "maxWidth",
  "minHeight",
  "maxHeight",
  "rx",
  "ry",
  "r",
  "opacity",
  "fillOpacity",
  "strokeOpacity",
  "color",
  "bgcolor",
  "align",
  "size",
  "fontSize",
  "fontFamily",
  "fontWeight",
  "letterSpacing",
  "textAnchor",
  "dominantBaseline",
  "transform",
  "border",
  "cellPadding",
  "cellSpacing",
];
const factories = ["createElement", "cloneElement", "jsx", "jsxs", "jsxDEV"];
const reservedObject = ["className", "class", "style"];
export const boundaryRules = {
  "no-dom-styling": rule(
    "DOM으로 스타일을 읽거나 바꾸는 경로는 허용 파일 밖에서 금지다.",
    (ctx) => ({
      MemberExpression(node) {
        if (
          DOM_STYLE_MEMBERS.includes(propertyName(node.property)) ||
          (node.computed &&
            (node.property.type === "Identifier" ||
              propertyName(node.property) === undefined))
        )
          report(ctx, node);
      },
      Property(node) {
        if (
          node.parent.type === "ObjectPattern" &&
          DOM_STYLE_MEMBERS.includes(propertyName(node.key))
        )
          report(ctx, node);
      },
      Identifier(node) {
        if (node.name === "CSSStyleSheet") report(ctx, node);
      },
      JSXAttribute(node) {
        if (propertyName(node.name) === "dangerouslySetInnerHTML")
          report(ctx, node);
      },
      JSXOpeningElement(node) {
        if (["style", "link"].includes(propertyName(node.name)))
          report(ctx, node);
      },
    }),
  ),
  "no-element-factory": rule("요소 팩터리 대신 JSX만 쓴다.", (ctx) => ({
    MemberExpression(node) {
      if (factories.includes(propertyName(node.property))) report(ctx, node);
    },
    Property(node) {
      if (factories.includes(propertyName(node.key))) report(ctx, node);
    },
    Identifier(node) {
      if (factories.includes(node.name)) report(ctx, node);
    },
    Literal(node) {
      if (factories.includes(node.value)) report(ctx, node);
    },
    TemplateElement(node) {
      if (factories.includes(node.value.cooked)) report(ctx, node);
    },
  })),
  "no-style-object": rule(
    "객체의 className, class, style과 동적 키는 금지다.",
    (ctx) => ({
      Property(node) {
        if (
          node.parent.type === "ObjectExpression" &&
          (node.computed || reservedObject.includes(propertyName(node.key)))
        )
          report(ctx, node);
      },
      MemberExpression(node) {
        if (propertyName(node.property) === "fromEntries") report(ctx, node);
      },
      Identifier(node) {
        if (node.name === "fromEntries") report(ctx, node);
      },
      Literal(node) {
        if (node.value === "fromEntries") report(ctx, node);
      },
    }),
  ),
  "no-presentation-props": rule(
    "표현 속성 대신 토큰 클래스를 쓴다.",
    (ctx) => ({
      JSXAttribute(node) {
        const tag = node.parent.name;
        if (
          tag.type === "JSXIdentifier" &&
          /^[a-z]/u.test(tag.name) &&
          PRESENTATION_PROPS.some(
            (p) =>
              p.replaceAll("-", "").toLowerCase() ===
              propertyName(node.name)?.replaceAll("-", "").toLowerCase(),
          )
        )
          report(ctx, node);
      },
    }),
  ),
  "no-shared-jsx": rule(
    "shared/ds 밖의 shared는 JSX를 렌더하지 않는다.",
    (ctx) => ({
      JSXElement(node) {
        report(ctx, node);
      },
      JSXFragment(node) {
        report(ctx, node);
      },
    }),
  ),
};
