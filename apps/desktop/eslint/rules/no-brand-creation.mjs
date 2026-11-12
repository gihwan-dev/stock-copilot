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

const isLiteralOnly = (node) =>
  (node.type === "Literal" && typeof node.value === "string") ||
  (node.type === "TemplateLiteral" && node.expressions.every(isLiteralOnly)) ||
  (node.type === "BinaryExpression" &&
    node.operator === "+" &&
    isLiteralOnly(node.left) &&
    isLiteralOnly(node.right));

const isStringLiteralUnion = (type) =>
  type.isUnion()
    ? type.types.every(isStringLiteralUnion)
    : (type.flags & F.StringLiteral) !== 0;

const isCatalogObject = (node) =>
  node.type === "ObjectExpression" &&
  node.properties.every(
    (property) =>
      property.type === "Property" &&
      !property.computed &&
      (isCatalogObject(property.value) ||
        (property.value.type === "TSAsExpression" &&
          isLiteralOnly(property.value.expression))),
  );

const isConstCatalog = (node) =>
  node?.type === "TSAsExpression" &&
  node.typeAnnotation.type === "TSTypeReference" &&
  node.typeAnnotation.typeName.name === "const" &&
  isCatalogObject(node.expression);

const memberRoot = (node) =>
  node.type === "MemberExpression" ? memberRoot(node.object) : node;

const memberKeys = (node) =>
  node.type === "MemberExpression"
    ? [...memberKeys(node.object), ...(node.computed ? [node.property] : [])]
    : [];

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
      messagesShape:
        "messages.ts에는 문구 목록(키마다 언어별 리터럴을 as UiText로 단언한 as const 객체)과 그 목록을 리터럴 유니온 키로 고르는 함수만 둔다.",
      assertion: "허용 목록 파일에서도 단언은 브랜드 타입으로만 쓴다.",
    },
  },
  create: (context) => {
    const [{ mode }] = context.options;
    const services = context.sourceCode.parserServices;
    const checker = services.program.getTypeChecker();
    const tsNodeOf = (node) => services.esTreeNodeToTSNodeMap.get(node);
    const report = (node, messageId) => context.report({ node, messageId });

    const isZodBrand = (symbol) =>
      symbol !== undefined &&
      symbol.escapedName.toString() === "brand" &&
      declaredInZod(symbol);

    const brandCall = (node) => {
      const declaration = checker
        .getResolvedSignature(tsNodeOf(node))
        ?.getDeclaration();
      return isZodBrand(declaration?.symbol);
    };

    const checkBrandCall = (call) => {
      if (mode === "forbid") return report(call, "forbidden");
      if (mode === "messages") return report(call, "messagesOnly");
      const name = literalName(call);
      if (name === undefined) return report(call, "unnamed");
      if (name === UI_TEXT) return report(call, "uiText");
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
          !isLiteralOnly(node.expression))
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

    const catalogs = new Set();

    const isSelector = (declarator) => {
      const init = declarator.init;
      if (init?.type !== "ArrowFunctionExpression") return false;
      const params = init.params.map((param) => param.name);
      return (
        init.params.every(
          (param) =>
            param.type === "Identifier" &&
            isStringLiteralUnion(services.getTypeAtLocation(param)),
        ) &&
        init.body.type === "MemberExpression" &&
        catalogs.has(memberRoot(init.body).name) &&
        memberKeys(init.body).every(
          (key) => key.type === "Identifier" && params.includes(key.name),
        )
      );
    };

    const isCatalogDeclaration = (declaration) =>
      declaration.kind === "const" &&
      declaration.declarations.length === 1 &&
      declaration.declarations[0].id.type === "Identifier" &&
      isConstCatalog(declaration.declarations[0].init);

    const checkMessagesFile = (program) => {
      program.body.forEach((statement) => {
        const inner =
          statement.type === "ExportNamedDeclaration" &&
          statement.declaration !== null
            ? statement.declaration
            : statement;
        if (
          (inner.type === "ImportDeclaration" &&
            (inner.importKind === "type" ||
              inner.specifiers.every(
                (specifier) => specifier.importKind === "type",
              ))) ||
          inner.type === "TSTypeAliasDeclaration"
        )
          return;
        if (
          inner.type === "VariableDeclaration" &&
          isCatalogDeclaration(inner)
        ) {
          catalogs.add(inner.declarations[0].id.name);
          return;
        }
        if (
          inner.type === "VariableDeclaration" &&
          inner.kind === "const" &&
          inner.declarations.length === 1 &&
          isSelector(inner.declarations[0])
        )
          return;
        report(statement, "messagesShape");
      });
    };

    return {
      ...(mode === "messages" ? { Program: checkMessagesFile } : {}),
      MemberExpression: (node) => {
        const name = node.computed
          ? node.property.type === "Literal"
            ? node.property.value
            : undefined
          : node.property.name;
        const isCallee =
          node.parent.type === "CallExpression" && node.parent.callee === node;
        if (
          !isCallee &&
          isZodBrand(services.getTypeAtLocation(node).getSymbol())
        )
          report(node, mode === "forbid" ? "forbidden" : "unnamed");
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
      CallExpression: (node) => {
        if (brandCall(node)) checkBrandCall(node);
        checkZodTypeArguments(node);
      },
    };
  },
};
