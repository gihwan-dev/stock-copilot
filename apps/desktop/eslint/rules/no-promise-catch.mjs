import { declaredInLibrary } from "../types.mjs";

const libraryMethod = (symbol, name) =>
  symbol !== undefined &&
  symbol.escapedName.toString() === name &&
  declaredInLibrary(symbol);

export const noPromiseCatch = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Disallow Promise catch, two-argument then and allSettled, judged by the resolved declaration",
    },
    schema: [],
    messages: {
      catch:
        "catch 금지: 거절을 Result로 바꾸는 곳은 shared/ipc 호출 어댑터와 shared/platform/detach뿐이다.",
      then: "두 인자 then 금지: 거절 처리기는 shared/ipc 호출 어댑터와 shared/platform/detach에서만 붙인다.",
      allSettled:
        "Promise.allSettled 금지: 거절을 값으로 바꾸는 길이다. shared/ipc 호출 어댑터와 shared/platform/detach에서만.",
    },
  },
  create: (context) => {
    const services = context.sourceCode.parserServices;
    const checker = services.program.getTypeChecker();
    const tsNodeOf = (node) => services.esTreeNodeToTSNodeMap.get(node);
    const report = (node, messageId) => context.report({ node, messageId });

    const checkValue = (node, at) => {
      const symbol = services.getTypeAtLocation(node).getSymbol();
      if (libraryMethod(symbol, "catch")) report(at, "catch");
      if (libraryMethod(symbol, "allSettled")) report(at, "allSettled");
    };

    return {
      MemberExpression: (node) => checkValue(node, node),
      "ObjectPattern > Property": (node) => checkValue(node.value, node),
      CallExpression: (node) => {
        const declaration = checker
          .getResolvedSignature(tsNodeOf(node))
          ?.getDeclaration();
        if (
          libraryMethod(declaration?.symbol, "then") &&
          (node.arguments.length > 1 ||
            node.arguments.some(
              (argument) => argument.type === "SpreadElement",
            ))
        )
          report(node, "then");
      },
    };
  },
};
