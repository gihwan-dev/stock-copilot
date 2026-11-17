import { projectPath, typeFlags } from "./types.mjs";
const nativeTags = (ctx, name) => {
  if (name.type === "JSXIdentifier" && /^[a-z]/u.test(name.name))
    return [name.name];
  const type = ctx.sourceCode.parserServices.getTypeAtLocation?.(name);
  const parts = type?.isUnion() ? type.types : [type];
  return parts
    .filter((part) => (part?.flags & typeFlags.StringLiteral) !== 0)
    .map((part) => part.value);
};
const wrappingClass = (node) =>
  node.attributes.some((attribute) => {
    if (
      attribute.type !== "JSXAttribute" ||
      attribute.name.name !== "className"
    )
      return false;
    const value =
      attribute.value?.type === "JSXExpressionContainer"
        ? attribute.value.expression
        : attribute.value;
    return (
      value?.type === "Literal" &&
      typeof value.value === "string" &&
      value.value.split(/\s+/u).includes("chat-content")
    );
  });
export const nativeOverflowElement = (ctx, node) => {
  const names = nativeTags(ctx, node.name);
  return (
    names.includes("dialog") ||
    (names.includes("pre") && !wrappingClass(node)) ||
    (names.includes("textarea") &&
      projectPath(ctx, ctx.filename) !== "src/shared/ds/ChatInput.tsx")
  );
};
export const nativeOverflowAttribute = (ctx, node, key) => {
  if (nativeTags(ctx, node.parent.name).length === 0) return false;
  return (
    [
      "popover",
      "popoverTarget",
      "popoverTargetAction",
      "cols",
      "rows",
    ].includes(key) ||
    (key === "wrap" && !["soft", "hard"].includes(node.value?.value))
  );
};
