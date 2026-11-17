import { relative, sep } from "node:path";
const owners = {
  "data-scroll-viewport": ["src/shared/ds/ScrollArea.tsx"],
  "data-truncate": ["src/shared/ds/Text.tsx", "src/shared/ds/Heading.tsx"],
};

const text = (node) => {
  if (node?.type === "Identifier" || node?.type === "JSXIdentifier")
    return node.name;
  if (node?.type === "Literal" && typeof node.value === "string")
    return node.value;
  if (node?.type === "TemplateLiteral" && node.expressions.length === 0)
    return node.quasis[0].value.cooked;
  if (node?.type === "BinaryExpression" && node.operator === "+") {
    const left = text(node.left),
      right = text(node.right);
    return left === undefined || right === undefined ? undefined : left + right;
  }
  return undefined;
};

export const overflowMarkers = {
  meta: {
    type: "problem",
    schema: [],
    messages: { forbidden: "넘침 검사 표식은 소유 컴포넌트에서만 쓴다." },
  },
  create(context) {
    const file = relative(context.cwd, context.filename).split(sep).join("/");
    const check = (node, key) => {
      const marker = text(key);
      if (Object.hasOwn(owners, marker) && !owners[marker].includes(file))
        context.report({ node, messageId: "forbidden" });
    };
    return {
      JSXAttribute(node) {
        check(node, node.name);
      },
      Property(node) {
        check(node, node.key);
      },
    };
  },
};
