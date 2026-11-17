import { relative, sep } from "node:path";
const owners = {
  "data-scroll-viewport": ["src/shared/ds/ScrollArea.tsx"],
  "data-truncate": ["src/shared/ds/Text.tsx", "src/shared/ds/Heading.tsx"],
};

const text = (node, resolve, seen) => {
  if (node?.type === "Identifier") return resolve(node, seen);
  if (node?.type === "JSXIdentifier") return node.name;
  if (node?.type === "Literal" && typeof node.value === "string")
    return node.value;
  if (node?.type === "TemplateLiteral") {
    const parts = node.quasis.map((item) => item.value.cooked);
    const values = node.expressions.map((item) => text(item, resolve, seen));
    if (parts.some((item) => typeof item !== "string")) return undefined;
    if (values.some((item) => item === undefined)) return undefined;
    return parts.map((part, index) => part + (values[index] ?? "")).join("");
  }
  if (node?.type === "BinaryExpression" && node.operator === "+") {
    const left = text(node.left, resolve, seen),
      right = text(node.right, resolve, seen);
    return left === undefined || right === undefined ? undefined : left + right;
  }
  if (
    node?.type === "CallExpression" &&
    node.callee.type === "MemberExpression" &&
    !node.callee.computed &&
    node.callee.property.name === "join" &&
    node.callee.object.type === "ArrayExpression" &&
    node.arguments.length <= 1
  ) {
    const separator =
      node.arguments.length === 0
        ? ","
        : text(node.arguments[0], resolve, seen);
    const values = node.callee.object.elements.map((item) =>
      text(item, resolve, seen),
    );
    return separator === undefined || values.some((item) => item === undefined)
      ? undefined
      : values.join(separator);
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
    const resolve = (node, seen) => {
      for (
        let scope = context.sourceCode.getScope(node);
        scope !== null;
        scope = scope.upper
      ) {
        const variable = scope.set.get(node.name);
        if (variable === undefined) continue;
        if (seen.has(variable) || variable.defs.length !== 1) return undefined;
        const definition = variable.defs[0];
        if (
          definition.type !== "Variable" ||
          definition.parent.kind !== "const" ||
          definition.node.id.type !== "Identifier" ||
          definition.node.init === null
        )
          return undefined;
        return text(
          definition.node.init,
          resolve,
          new Set([...seen, variable]),
        );
      }
      return undefined;
    };
    const check = (node, key, computed) => {
      const marker =
        !computed && key.type === "Identifier"
          ? key.name
          : text(key, resolve, new Set());
      if (Object.hasOwn(owners, marker) && !owners[marker].includes(file))
        context.report({ node, messageId: "forbidden" });
    };
    return {
      JSXAttribute(node) {
        check(node, node.name, false);
      },
      Property(node) {
        check(node, node.key, node.computed);
      },
    };
  },
};
