import { parse } from "postcss";
const contextOf = (node) =>
  node.parent?.type === "root"
    ? []
    : [
        ...contextOf(node.parent),
        node.parent.type === "atrule"
          ? `@${node.parent.name} ${node.parent.params}`.trim()
          : node.parent.selector,
      ];
export const collectRules = (css) => {
  const rows = [];
  parse(css).walkRules((node) =>
    rows.push({ selector: node.selector, context: contextOf(node) }),
  );
  return [...new Set(rows.map((r) => JSON.stringify(r)))]
    .sort()
    .map((r) => JSON.parse(r));
};
