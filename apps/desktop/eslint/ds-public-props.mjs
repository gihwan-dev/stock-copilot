import ts from "typescript";
import { propertyName } from "./ds-boundaries.mjs";
import { CHART_NUMBER_ALLOWLIST } from "./allowlists.mjs";

const optional = (type) =>
  (type.flags & (ts.TypeFlags.Undefined | ts.TypeFlags.Null)) !== 0;
const from = (symbol, pattern) =>
  (symbol?.declarations ?? []).some((d) =>
    pattern.test(d.getSourceFile().fileName),
  );
const tokenProperty =
  /^(textColor|bg|tone|gap|rowGap|columnGap|spacing|padding(?:Top|Bottom|Left|Right|Inline|Block|X|Y)?|margin(?:Top|Bottom|Left|Right|Inline|Block|X|Y)?|[pm][xytrblse]?|color|background|backgroundColor|borderColor|outlineColor|fill|stroke|size|radius|borderRadius|borderWidth|font|fontFamily|typography|textStyle|textSize|lineHeight|letterSpacing|fontWeight|fontSize|width|height|minWidth|maxWidth|minHeight|maxHeight|shadow|opacity|zIndex|duration|easing)$/u;
const reactType = (type, names) =>
  names.includes(type.getSymbol()?.name) &&
  from(type.getSymbol(), /\/@types\/react\/index\.d\.ts$/u);
const generatedReference = (checker, node, seen = new Set()) => {
  if (node === undefined || seen.has(node)) return false;
  seen.add(node);
  if (/\/src\/shared\/ds\/tokens\.ts$/u.test(node.getSourceFile().fileName))
    return true;
  if (ts.isTypeReferenceNode(node)) {
    const symbol = checker.getSymbolAtLocation(node.typeName);
    const resolved =
      symbol && (symbol.flags & ts.SymbolFlags.Alias) !== 0
        ? checker.getAliasedSymbol(symbol)
        : symbol;
    if (
      (resolved?.declarations ?? []).some((d) =>
        generatedReference(checker, d, seen),
      )
    )
      return true;
  }
  return (
    ts.forEachChild(
      node,
      (child) => generatedReference(checker, child, seen) || undefined,
    ) === true
  );
};
export const publicProps = {
  meta: {
    type: "problem",
    schema: [],
    messages: {
      forbidden:
        "공개 props는 닫힌 값 계약이다. HTML 전달·DOM 이벤트·콜백 ref·자유 스타일 대신 토큰 이름과 값만 받는다.",
    },
  },
  create(ctx) {
    const services = ctx.sourceCode.parserServices;
    const checker = services.program?.getTypeChecker();
    if (checker === undefined) return {};
    const report = (node) => ctx.report({ node, messageId: "forbidden" });
    const tokenNames = new Set();
    if (checker !== undefined)
      for (const file of services.program.getSourceFiles()) {
        if (!/\/src\/shared\/ds\/tokens\.ts$/u.test(file.fileName)) continue;
        const symbol = checker.getSymbolAtLocation(file);
        if (symbol !== undefined)
          for (const entry of checker.getExportsOfModule(symbol)) {
            const type = checker.getDeclaredTypeOfSymbol(entry);
            for (const item of type.isUnion() ? type.types : [type])
              if ((item.flags & ts.TypeFlags.StringLiteral) !== 0)
                tokenNames.add(item.value);
          }
      }
    const tokenValue = (type) =>
      type.isUnion()
        ? type.types.every(tokenValue)
        : optional(type) ||
          ((type.flags & ts.TypeFlags.StringLiteral) !== 0 &&
            tokenNames.has(type.value));
    const library = (type) =>
      from(
        type.getSymbol(),
        /\/(?:lib\.[^/]+\.d\.ts|@types\/react\/index\.d\.ts)$/u,
      );
    const argumentsOf = (type) => [
      ...(type.aliasTypeArguments ?? []),
      ...((type.objectFlags & ts.ObjectFlags.Reference) !== 0
        ? checker.getTypeArguments(type)
        : []),
    ];
    const layoutValue = (type) => {
      if (type.isUnion()) return type.types.every(layoutValue);
      if (optional(type)) return true;
      if ((type.flags & ts.TypeFlags.NumberLiteral) !== 0)
        return [3, 4, 6, 8, 9, 12].includes(type.value);
      if (
        (type.flags & ts.TypeFlags.Object) === 0 ||
        checker.getIndexInfosOfType(type).length !== 0
      )
        return false;
      const properties = checker.getPropertiesOfType(type);
      return (
        properties.length > 0 &&
        properties.every(
          (p) =>
            ["narrow", "medium", "wide"].includes(p.name) &&
            layoutValue(checker.getTypeOfSymbol(p)),
        )
      );
    };
    const tokenCategory = (name) => {
      if (/color|^bg$|^tone$|background|fill|stroke/iu.test(name))
        return "ColorToken";
      if (/gap|spacing|padding|margin|^[pm][xytrblse]?$/u.test(name))
        return "SpacingToken";
      if (/radius/iu.test(name)) return "RadiusToken";
      if (name === "fontFamily") return "FontToken";
      return undefined;
    };
    const categoryNames = new Map();
    for (const file of services.program.getSourceFiles()) {
      if (!/\/src\/shared\/ds\/tokens\.ts$/u.test(file.fileName)) continue;
      const symbol = checker.getSymbolAtLocation(file);
      if (symbol === undefined) continue;
      for (const entry of checker.getExportsOfModule(symbol)) {
        const type = checker.getDeclaredTypeOfSymbol(entry);
        categoryNames.set(
          entry.name,
          new Set((type.isUnion() ? type.types : [type]).map((t) => t.value)),
        );
      }
    }
    const propertyBad = (p, seen, depth) => {
      const value = checker.getTypeOfSymbol(p);
      if (["className", "style"].includes(p.name)) return true;
      if (p.name === "ref")
        return (value.isUnion() ? value.types : [value]).some(
          (t) =>
            (t.flags & ts.TypeFlags.Undefined) === 0 &&
            !reactType(t, ["RefObject"]),
        );
      if (tokenProperty.test(p.name)) {
        const category = categoryNames.get(tokenCategory(p.name));
        const values = value.isUnion() ? value.types : [value];
        return (
          !tokenValue(value) ||
          (category !== undefined &&
            values.some((t) => !optional(t) && !category.has(t.value))) ||
          !(p.declarations ?? []).some((d) =>
            generatedReference(checker, d.type),
          )
        );
      }
      if (["columns", "span"].includes(p.name)) return !layoutValue(value);
      if (
        /^(amount|money|price)$/u.test(p.name) &&
        (value.isUnion() ? value.types : [value]).some(
          (t) => !optional(t) && (t.flags & ts.TypeFlags.Object) === 0,
        )
      )
        return true;
      return badValue(value, seen, depth + 1);
    };
    const badValue = (type, seen = new Set(), depth = 0) => {
      if (seen.has(type)) return false;
      if (depth > 32) return true;
      seen.add(type);
      if (optional(type)) return false;
      if (
        type.aliasSymbol?.name === "ReactNode" &&
        from(type.aliasSymbol, /\/@types\/react\/index\.d\.ts$/u)
      )
        return false;
      if (type.isUnionOrIntersection())
        return type.types.some((t) => badValue(t, seen, depth + 1));
      if ((type.flags & ts.TypeFlags.TypeParameter) !== 0) {
        const constraint = checker.getBaseConstraintOfType(type);
        return (
          constraint !== undefined && badValue(constraint, seen, depth + 1)
        );
      }
      if (
        (type.flags &
          (ts.TypeFlags.Any |
            ts.TypeFlags.Unknown |
            ts.TypeFlags.Number |
            ts.TypeFlags.BigIntLike)) !==
        0
      )
        return true;
      if (
        from(
          type.getSymbol(),
          /\/lib\.dom(?:\.iterable|\.asynciterable)?\.d\.ts$/u,
        )
      )
        return true;
      if (
        from(type.getSymbol(), /\/@types\/react\/index\.d\.ts$/u) &&
        /Event$/u.test(type.getSymbol()?.name ?? "")
      )
        return true;
      if (reactType(type, ["ReactElement", "ReactPortal"])) return false;
      if (reactType(type, ["FunctionComponent", "ComponentClass"])) {
        const props = argumentsOf(type)[0];
        return props !== undefined && badProps(props, seen, depth + 1);
      }
      if (checker.isArrayType(type) || checker.isTupleType(type))
        return checker
          .getTypeArguments(type)
          .some((t) => badValue(t, seen, depth + 1));
      if (library(type))
        return (
          !["Array", "ReadonlyArray"].includes(type.getSymbol()?.name) &&
          argumentsOf(type).some((t) => badValue(t, seen, depth + 1))
        );
      if (
        (type.aliasTypeArguments ?? []).some((t) =>
          badValue(t, seen, depth + 1),
        )
      )
        return true;
      if (
        (type.flags &
          (ts.TypeFlags.StringLike |
            ts.TypeFlags.BooleanLike |
            ts.TypeFlags.ESSymbolLike |
            ts.TypeFlags.NumberLiteral)) !==
        0
      )
        return false;
      const signatures = type.getCallSignatures();
      if (signatures.length !== 0)
        return signatures.some(
          (s) =>
            s
              .getParameters()
              .some((p) =>
                badValue(checker.getTypeOfSymbol(p), seen, depth + 1),
              ) || badValue(s.getReturnType(), seen, depth + 1),
        );
      if ((type.flags & ts.TypeFlags.Object) === 0) return false;
      if (checker.getIndexInfosOfType(type).length !== 0) return true;
      return checker
        .getPropertiesOfType(type)
        .some((p) => propertyBad(p, seen, depth + 1));
    };
    const genericProps = (type, seen = new Set()) => {
      if (seen.has(type)) return false;
      seen.add(type);
      return (
        (type.flags & ts.TypeFlags.TypeParameter) !== 0 ||
        (type.types ?? []).some((t) => genericProps(t, seen)) ||
        (type.aliasTypeArguments ?? []).some((t) => genericProps(t, seen))
      );
    };
    const badProps = (type, seen = new Set(), depth = 0) => {
      if (depth > 32) return true;
      if (type.isUnionOrIntersection())
        return type.types.some((t) => badProps(t, seen, depth + 1));
      if (
        (type.flags &
          (ts.TypeFlags.TypeParameter |
            ts.TypeFlags.Any |
            ts.TypeFlags.Unknown)) !==
          0 ||
        genericProps(type)
      )
        return true;
      if (checker.getIndexInfosOfType(type).length !== 0) return true;
      return checker
        .getPropertiesOfType(type)
        .some((p) => propertyBad(p, seen, depth + 1));
    };
    const badExport = (type, seen = new Set(), depth = 0) => {
      if (seen.has(type)) return false;
      if (depth > 8) return true;
      seen.add(type);
      if (type.isUnionOrIntersection())
        return type.types.some((t) => badExport(t, seen, depth + 1));
      if (
        type.getCallSignatures().some((signature) => {
          const props = signature.getParameters()[0];
          return (
            props !== undefined && badProps(checker.getTypeOfSymbol(props))
          );
        })
      )
        return true;
      if (library(type) || (type.flags & ts.TypeFlags.Object) === 0)
        return false;
      return checker
        .getPropertiesOfType(type)
        .some((p) => badExport(checker.getTypeOfSymbol(p), seen, depth + 1));
    };
    const openGeneric = (type, seen = new Set()) => {
      if (seen.has(type)) return false;
      seen.add(type);
      if ((type.flags & ts.TypeFlags.TypeParameter) !== 0)
        return checker.getBaseConstraintOfType(type) === undefined;
      if (type.isUnionOrIntersection())
        return type.types.some((t) => openGeneric(t, seen));
      if (argumentsOf(type).some((t) => openGeneric(t, seen))) return true;
      if (library(type) || (type.flags & ts.TypeFlags.Object) === 0)
        return false;
      return checker
        .getPropertiesOfType(type)
        .some((p) => openGeneric(checker.getTypeOfSymbol(p), seen));
    };
    return {
      "Program:exit"(node) {
        if (checker === undefined) return;
        const source = services.esTreeNodeToTSNodeMap.get(node);
        const module = checker.getSymbolAtLocation(source);
        if (module === undefined) return;
        for (const exported of checker.getExportsOfModule(module)) {
          if (!/^[A-Z]/u.test(exported.name)) continue;
          const type = checker.getTypeOfSymbolAtLocation(exported, source);
          if (badExport(type)) report(node);
        }
      },
      RestElement(node) {
        if (node.parent.type === "ObjectPattern") report(node);
      },
      JSXSpreadAttribute: report,
      JSXAttribute(node) {
        if (
          node.value?.type === "JSXExpressionContainer" &&
          openGeneric(services.getTypeAtLocation(node.value.expression))
        )
          return report(node);
        if (
          node.parent.name.type !== "JSXMemberExpression" &&
          !/^[A-Z]/u.test(propertyName(node.parent.name) ?? "")
        )
          return;
        const filename = ctx.filename.replaceAll(String.fromCharCode(92), "/");
        if (
          CHART_NUMBER_ALLOWLIST.some((file) => filename.endsWith("/" + file))
        )
          return;
        if (
          node.value?.type === "Literal" &&
          typeof node.value.value === "string"
        ) {
          if (
            tokenProperty.test(node.name.name) &&
            !tokenNames.has(node.value.value)
          )
            report(node);
          return;
        }
        const expression =
          node.value?.type === "JSXExpressionContainer"
            ? node.value.expression
            : node.value;
        if (expression === undefined || expression === null) return;
        const type = services.getTypeAtLocation(expression);
        if (["columns", "span"].includes(node.name.name) && layoutValue(type))
          return;
        if (tokenProperty.test(node.name.name) && !tokenValue(type))
          return report(node);
        const contextual = checker.getContextualType(
          services.esTreeNodeToTSNodeMap.get(expression),
        );
        if (
          (badValue(type) ||
            (contextual !== undefined && badValue(contextual))) &&
          !["ref", "children"].includes(node.name.name)
        )
          report(node);
      },
      CallExpression(node) {
        if (
          propertyName(node.callee) === "forwardRef" ||
          propertyName(node.callee.property) === "forwardRef"
        )
          report(node);
      },
    };
  },
};
