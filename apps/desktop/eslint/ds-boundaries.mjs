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
export const intlOptions = (node) => {
  const call = node.parent?.parent;
  return (
    ["NewExpression", "CallExpression"].includes(call?.type) &&
    call.callee?.type === "MemberExpression" &&
    call.callee.object.name === "Intl" &&
    call.callee.property.name === "NumberFormat"
  );
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
        if (DOM_STYLE_MEMBERS.includes(propertyName(node.property)))
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
        if (
          [
            "style",
            "link",
            "animate",
            "animateTransform",
            "animateMotion",
            "set",
            "font",
            "center",
            "b",
            "i",
            "u",
            "iframe",
            "object",
            "embed",
            "foreignObject",
          ].includes(propertyName(node.name))
        )
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
          (node.computed || reservedObject.includes(propertyName(node.key))) &&
          !(propertyName(node.key) === "style" && intlOptions(node))
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
  "no-presentation-props": {
    meta: {
      type: "problem",
      schema: [
        {
          type: "object",
          properties: { geometry: { enum: ["allow", "forbid"] } },
          additionalProperties: false,
        },
      ],
      messages: {
        forbidden:
          "표현 속성은 토큰 클래스만, 기하는 보호된 아이콘 경로 파일에서만 쓴다.",
      },
    },
    create(ctx) {
      return {
        JSXAttribute(node) {
          const tag = node.parent.name;
          if (tag.type !== "JSXIdentifier" || !/^[a-z]/u.test(tag.name)) return;
          const key = propertyName(node.name);
          const geometry =
            [
              "d",
              "viewBox",
              "points",
              "x",
              "y",
              "cx",
              "cy",
              "r",
              "rx",
              "ry",
              "x1",
              "y1",
              "x2",
              "y2",
            ].includes(key) ||
            (["rect", "circle", "ellipse", "line"].includes(tag.name) &&
              ["width", "height"].includes(key));
          if (geometry && ctx.options[0]?.geometry === "allow") {
            const v = node.value;
            const valid = ["d", "viewBox", "points"].includes(key)
              ? v?.type === "Literal" && typeof v.value === "string"
              : v?.type === "JSXExpressionContainer" &&
                ((v.expression.type === "Literal" &&
                  typeof v.expression.value === "number") ||
                  (v.expression.type === "UnaryExpression" &&
                    v.expression.operator === "-" &&
                    typeof v.expression.argument.value === "number"));
            if (valid) return;
          }
          if (
            geometry ||
            PRESENTATION_PROPS.some(
              (p) =>
                p.replaceAll("-", "").toLowerCase() ===
                key?.replaceAll("-", "").toLowerCase(),
            )
          )
            report(ctx, node);
        },
      };
    },
  },
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
