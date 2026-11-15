import { semanticVariants } from "./ds-semantic-variants.mjs";
export const tokenTailwind = (plugin) => ({
  ...plugin,
  rules: {
    ...plugin.rules,
    ...Object.fromEntries(
      ["no-unknown-classes", "no-restricted-classes"].map((name) => [
        name,
        {
          ...plugin.rules[name],
          create(ctx) {
            const original = plugin.rules[name];
            const keyLiteral = (node) => {
              if (!node || node.type !== "Literal") return false;
              let child = node;
              for (
                let parent = node.parent;
                parent;
                child = parent, parent = parent.parent
              ) {
                if (
                  parent.type === "MemberExpression" &&
                  parent.computed &&
                  parent.property === child
                ) {
                  const value =
                    ctx.sourceCode.parserServices.esTreeNodeToTSNodeMap?.get(
                      parent,
                    );
                  return (
                    value !== undefined && semanticVariants(ctx).classes(value)
                  );
                }
                if (parent.type === "JSXAttribute") return false;
              }
              return false;
            };
            const proxy = Object.create(ctx, {
              report: {
                value: (descriptor) => {
                  const location =
                    descriptor.loc?.start ??
                    descriptor.loc ??
                    descriptor.node?.loc?.start;
                  const reported = location
                    ? ctx.sourceCode.getNodeByRangeIndex(
                        ctx.sourceCode.getIndexFromLoc(location),
                      )
                    : descriptor.node;
                  if (!keyLiteral(reported)) ctx.report(descriptor);
                },
              },
            });
            return original.create(proxy);
          },
        },
      ]),
    ),
  },
});
