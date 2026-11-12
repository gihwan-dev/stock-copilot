import { brandNamesOf, declaredInZod, typeFlags as F } from "../types.mjs";

const UI_TEXT = "UiText";
const BRAND_SYMBOLS = new Set(["$brand", "BRAND"]);

const literalName = (node) => {
  const typeArgument = node.typeArguments?.params[0];
  if (
    typeArgument?.type === "TSLiteralType" &&
    typeof typeArgument.literal.value === "string"
  )
    return typeArgument.literal.value;
  const [argument] = node.arguments;
  return argument?.type === "Literal" && typeof argument.value === "string"
    ? argument.value
    : undefined;
};

const isStringLiteralType = (type) =>
  type.isUnion()
    ? type.types.every(isStringLiteralType)
    : (type.flags & F.StringLiteral) !== 0;

export const noBrandCreation = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Allow brand creation (Zod .brand(), brand assertions) only in the brand allowlist, and UiText only in messages.ts",
    },
    schema: [
      {
        type: "object",
        properties: {
          mode: { enum: ["forbid", "brand-file", "messages"] },
        },
        required: ["mode"],
        additionalProperties: false,
      },
    ],
    messages: {
      forbidden:
        "브랜드는 shared/lib/brand, shared/lib/money, shared/lib/format, shared/ipc 스키마, 생성된 bindings.ts에서만 만든다.",
      uiText: "UiText는 messages.ts에서만 만든다.",
      messagesOnly: "messages.ts에서는 문자열 리터럴로 UiText만 만든다.",
      unnamed: "브랜드 이름은 리터럴 하나로 적는다.",
      assertion: "허용 목록 파일에서도 단언은 브랜드 타입으로만 쓴다.",
    },
  },
  create: (context) => {
    const [{ mode }] = context.options;
    const services = context.sourceCode.parserServices;
    const checker = services.program.getTypeChecker();
    const tsNodeOf = (node) => services.esTreeNodeToTSNodeMap.get(node);
    const report = (node, messageId) => context.report({ node, messageId });

    const zodMember = (node) =>
      declaredInZod(checker.getSymbolAtLocation(tsNodeOf(node).name));

    const checkBrandCall = (member) => {
      const call = member.parent;
      if (mode === "forbid") return report(member, "forbidden");
      if (call.type !== "CallExpression" || call.callee !== member)
        return report(member, "unnamed");
      const name = literalName(call);
      if (name === undefined) return report(call, "unnamed");
      if (mode === "messages" && name !== UI_TEXT)
        return report(call, "messagesOnly");
      if (mode === "brand-file" && name === UI_TEXT)
        return report(call, "uiText");
      return undefined;
    };

    const checkAssertion = (node) => {
      const annotation = node.typeAnnotation;
      if (
        annotation.type === "TSTypeReference" &&
        annotation.typeName.type === "Identifier" &&
        annotation.typeName.name === "const"
      )
        return;
      const names = brandNamesOf(
        checker,
        checker.getTypeFromTypeNode(tsNodeOf(annotation)),
      );
      if (mode === "forbid") {
        if (names.length > 0) report(node, "forbidden");
        return;
      }
      if (names.length === 0) return report(node, "assertion");
      if (mode === "brand-file" && names.includes(UI_TEXT))
        return report(node, "uiText");
      if (
        mode === "messages" &&
        (names.some((name) => name !== UI_TEXT) ||
          !isStringLiteralType(services.getTypeAtLocation(node.expression)))
      )
        return report(node, "messagesOnly");
      return undefined;
    };

    const checkZodTypeArguments = (node) => {
      if (mode !== "forbid" || node.typeArguments === undefined) return;
      const callee = tsNodeOf(node).expression;
      const symbol = checker.getSymbolAtLocation(callee.name ?? callee);
      if (!declaredInZod(symbol)) return;
      if (
        node.typeArguments.params.some(
          (param) =>
            brandNamesOf(checker, checker.getTypeFromTypeNode(tsNodeOf(param)))
              .length > 0,
        )
      )
        report(node, "forbidden");
    };

    return {
      MemberExpression: (node) => {
        const name = node.computed
          ? node.property.type === "Literal"
            ? node.property.value
            : undefined
          : node.property.name;
        if (name === "brand" && zodMember(node)) checkBrandCall(node);
        if (mode === "forbid" && BRAND_SYMBOLS.has(name))
          report(node, "forbidden");
      },
      "ObjectPattern > Property": (node) => {
        const key =
          node.key.type === "Identifier" ? node.key.name : node.key.value;
        if (key !== "brand") return;
        const property = services
          .getTypeAtLocation(node.parent)
          .getProperty("brand");
        if (declaredInZod(property))
          report(node, mode === "forbid" ? "forbidden" : "unnamed");
      },
      TSQualifiedName: (node) => {
        if (mode === "forbid" && BRAND_SYMBOLS.has(node.right.name))
          report(node, "forbidden");
      },
      ImportSpecifier: (node) => {
        const imported =
          node.imported.type === "Identifier"
            ? node.imported.name
            : node.imported.value;
        if (mode === "forbid" && BRAND_SYMBOLS.has(imported))
          report(node, "forbidden");
      },
      "TSAsExpression, TSTypeAssertion": checkAssertion,
      CallExpression: checkZodTypeArguments,
    };
  },
};
