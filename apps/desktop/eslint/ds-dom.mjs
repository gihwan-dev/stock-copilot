import { propertyName } from "./ds-boundaries.mjs";
import ts from "typescript";

const domSource = (symbol) =>
  (symbol?.declarations ?? []).some((d) =>
    [
      "lib.dom.d.ts",
      "lib.dom.iterable.d.ts",
      "lib.dom.asynciterable.d.ts",
    ].includes(
      d
        .getSourceFile()
        .fileName.replaceAll(String.fromCharCode(92), "/")
        .split("/")
        .at(-1),
    ),
  );
const domType = (checker, type, seen = new Set()) => {
  if (seen.has(type)) return false;
  seen.add(type);
  if (domSource(type.getSymbol())) return true;
  const nested = [
    ...(type.types ?? []),
    ...(type.aliasTypeArguments ?? []),
    ...((type.objectFlags & ts.ObjectFlags.Reference) !== 0
      ? checker.getTypeArguments(type)
      : []),
  ];
  return nested.some((t) => domType(checker, t, seen));
};
const primitiveValue = (type) =>
  type.isUnion()
    ? type.types.every(primitiveValue)
    : type.isIntersection()
      ? type.types.some(primitiveValue)
      : (type.flags &
          (ts.TypeFlags.StringLike |
            ts.TypeFlags.NumberLike |
            ts.TypeFlags.BigIntLike |
            ts.TypeFlags.BooleanLike |
            ts.TypeFlags.ESSymbolLike |
            ts.TypeFlags.Null |
            ts.TypeFlags.Undefined |
            ts.TypeFlags.Void)) !==
        0;
const literalKeys = (type) =>
  type.isUnion()
    ? type.types.flatMap(literalKeys)
    : (type.flags & ts.TypeFlags.StringLiteral) !== 0
      ? [type.value]
      : [];
const skippedIdentifier = (node) =>
  node.parent?.type?.startsWith("TS") ||
  (node.parent?.type === "MemberExpression" &&
    node.parent.property === node &&
    !node.parent.computed) ||
  node.parent?.type === "ImportSpecifier";
export const noDomValues = {
  meta: {
    type: "problem",
    schema: [],
    messages: {
      forbidden:
        "DOM 값 접근은 보호된 허용 파일에서만 한다. ref에는 RefObject만 넘긴다.",
    },
  },
  create(ctx) {
    const services = ctx.sourceCode.parserServices;
    const checker = services.program?.getTypeChecker();
    const report = (node) => ctx.report({ node, messageId: "forbidden" });
    const typeOf = (node) =>
      checker === undefined ? undefined : services.getTypeAtLocation(node);
    const entry = (key, value) => {
      const names = [
        propertyName(key),
        ...(checker !== undefined ? literalKeys(typeOf(key)) : []),
      ];
      return names.some(
        (name) =>
          ["current", "currentTarget", "target", "nativeEvent"].includes(
            name,
          ) &&
          (checker === undefined ||
            name === "current" ||
            !primitiveValue(typeOf(value))),
      );
    };
    return {
      JSXAttribute(node) {
        if (node.name.name !== "ref") return;
        const value = node.value?.expression;
        if (value?.type !== "Identifier" || checker === undefined)
          return report(node);
        const type = typeOf(value);
        if (
          checker.getSignaturesOfType(type, ts.SignatureKind.Call).length !==
            0 ||
          checker.getPropertyOfType(type, "current") === undefined
        )
          report(node);
      },
      MemberExpression(node) {
        const type = typeOf(node.object);
        if (
          entry(node.property, node) ||
          (type !== undefined &&
            (domType(checker, type) || domType(checker, typeOf(node))))
        )
          report(node);
      },
      ObjectPattern(node) {
        const type = typeOf(node);
        if (
          node.properties.some(
            (p) => p.type === "Property" && entry(p.key, p.value),
          ) ||
          (type !== undefined && domType(checker, type))
        )
          report(node);
      },
      Identifier(node) {
        if (checker === undefined || skippedIdentifier(node)) return;
        if (
          node.parent?.type === "CallExpression" &&
          node.parent.arguments.includes(node)
        ) {
          const callee = services.esTreeNodeToTSNodeMap.get(node.parent.callee);
          const reference = checker.getSymbolAtLocation(callee);
          const symbol =
            reference && (reference.flags & ts.SymbolFlags.Alias) !== 0
              ? checker.getAliasedSymbol(reference)
              : reference;
          if (
            ["inputEventValue", "inputCommit"].includes(symbol?.name) &&
            (symbol?.declarations ?? []).some(
              (d) =>
                d.getSourceFile().fileName ===
                ctx.filename.replace(
                  /\/src\/.*$/u,
                  "/src/shared/platform/input.ts",
                ),
            )
          )
            return;
        }

        const type = typeOf(node);
        const isRead = ctx.sourceCode
          .getScope(node)
          .references.some((r) => r.identifier === node && r.isRead());
        const refProp =
          node.parent?.type === "JSXExpressionContainer" &&
          node.parent.parent?.type === "JSXAttribute" &&
          node.parent.parent.name.name === "ref";
        if (
          domSource(
            checker.getSymbolAtLocation(
              services.esTreeNodeToTSNodeMap.get(node),
            ),
          ) ||
          (isRead &&
            !refProp &&
            checker.getSignaturesOfType(type, ts.SignatureKind.Call).length ===
              0 &&
            domType(checker, type))
        )
          report(node);
      },
    };
  },
};

export const noDomExports = {
  meta: {
    type: "problem",
    schema: [],
    messages: {
      forbidden: "DOM 접근 허용 파일은 DOM 값을 공개 API로 내보내지 않는다.",
    },
  },
  create(ctx) {
    const services = ctx.sourceCode.parserServices;
    const checker = services.program?.getTypeChecker();
    const leaks = (
      type,
      seen = new Set(),
      depth = 0,
      inspectParameters = true,
    ) => {
      if (depth > 32) return true;
      if (seen.has(type)) return false;
      seen.add(type);
      if (domType(checker, type)) return true;
      const argumentsOf = [
        ...(type.aliasTypeArguments ?? []),
        ...((type.objectFlags & ts.ObjectFlags.Reference) !== 0
          ? checker.getTypeArguments(type)
          : []),
      ];
      if (argumentsOf.some((argument) => leaks(argument, seen, depth + 1)))
        return true;
      if (
        (type.types ?? []).some((part) =>
          leaks(part, seen, depth + 1, inspectParameters),
        )
      )
        return true;
      if (
        (type.getSymbol()?.declarations ?? []).some((d) =>
          /\/lib\.es[^/]*\.d\.ts$/u.test(d.getSourceFile().fileName),
        )
      )
        return false;
      if ((type.flags & ts.TypeFlags.Object) === 0) return false;
      return (
        type
          .getCallSignatures()
          .some(
            (signature) =>
              leaks(signature.getReturnType(), seen, depth + 1) ||
              (inspectParameters &&
                signature
                  .getParameters()
                  .some((parameter) =>
                    leaks(checker.getTypeOfSymbol(parameter), seen, depth + 1),
                  )),
          ) ||
        checker
          .getPropertiesOfType(type)
          .some((property) =>
            leaks(checker.getTypeOfSymbol(property), seen, depth + 1),
          )
      );
    };
    return {
      "Program:exit"(node) {
        if (checker === undefined) return;
        const source = services.esTreeNodeToTSNodeMap.get(node);
        const symbol = checker.getSymbolAtLocation(source);
        if (symbol === undefined) return;
        for (const exported of checker.getExportsOfModule(symbol)) {
          const type = checker.getTypeOfSymbolAtLocation(exported, source);
          if (leaks(type, new Set(), 0, false))
            ctx.report({ node, messageId: "forbidden" });
        }
      },
    };
  },
};
