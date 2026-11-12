import ts from "typescript";
import { declaredIn, projectPath, typeFlags as F } from "../types.mjs";

const COERCING_CALLEES = new Set([
  "Number",
  "String",
  "Boolean",
  "BigInt",
  "Symbol",
  "parseFloat",
  "parseInt",
]);
const COERCING_MEMBERS = new Set(["stringify", "parseFloat", "parseInt"]);
const NULLISH_COMPARISON = new Set(["===", "!==", "==", "!="]);
const COERCING_UNARY = new Set(["+", "-", "~", "!"]);
const OPEN_PARAMETER = F.Any | F.Unknown | F.NonPrimitive;

const isNullish = (node) =>
  (node.type === "Literal" && node.value === null) ||
  (node.type === "Identifier" && node.name === "undefined");

const calleeName = (callee) =>
  callee.type === "Identifier"
    ? callee.name
    : callee.type === "MemberExpression" && !callee.computed
      ? callee.property.name
      : undefined;

export const noMoneyCoercion = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Disallow arithmetic, comparison, string conversion and value extraction on opaque money values",
    },
    schema: [
      {
        type: "object",
        properties: {
          moneyFile: { type: "string" },
          keyReaders: { type: "array", items: { type: "string" } },
        },
        required: ["moneyFile", "keyReaders"],
        additionalProperties: false,
      },
    ],
    messages: {
      operator:
        "금액 값에 산술·비교·연결 금지. 계산은 Rust domain이 하고 화면은 shared/lib/format으로 표시만 한다.",
      template:
        "금액 값을 템플릿에 넣지 않는다. shared/lib/format으로 표시한다.",
      conversion:
        "금액 값을 Number·String·parseFloat·JSON.stringify 등으로 바꾸지 않는다.",
      member:
        "금액 값에서 toString·valueOf 등 선언되지 않은 멤버를 쓰지 않는다.",
      extraction:
        "금액의 비공개 키로 값을 꺼내는 일은 shared/lib/money.ts와 shared/lib/format.ts에서만 한다.",
    },
  },
  create: (context) => {
    const [{ moneyFile, keyReaders }] = context.options;
    const services = context.sourceCode.parserServices;
    const checker = services.program.getTypeChecker();
    const here = projectPath(context, context.filename);
    const readsKeys = here === moneyFile || keyReaders.includes(here);

    const isMoneyKey = (property) =>
      property.escapedName.toString().startsWith("__@") &&
      declaredIn(context, property, [moneyFile]);

    const isMoneyType = (type) => {
      const base = checker.getBaseConstraintOfType(type) ?? type;
      return base.isUnion() || base.isIntersection()
        ? base.types.some(isMoneyType)
        : checker.getPropertiesOfType(base).some(isMoneyKey);
    };
    const isMoney = (node) => isMoneyType(services.getTypeAtLocation(node));

    const openParameter = (call, index) => {
      const signature = checker.getResolvedSignature(
        services.esTreeNodeToTSNodeMap.get(call),
      );
      const parameters = signature?.getParameters() ?? [];
      const parameter =
        parameters[Math.min(index, Math.max(parameters.length - 1, 0))];
      if (parameter === undefined) return true;
      const type = checker.getTypeOfSymbol(parameter);
      const declaration = parameter.valueDeclaration;
      const rest =
        declaration !== undefined &&
        ts.isParameter(declaration) &&
        declaration.dotDotDotToken !== undefined;
      const element =
        rest && index >= parameters.length - 1
          ? (checker.getIndexTypeOfType(type, ts.IndexKind.Number) ?? type)
          : type;
      return (
        (element.flags & OPEN_PARAMETER) !== 0 ||
        (element.isUnion() &&
          element.types.some((part) => (part.flags & OPEN_PARAMETER) !== 0)) ||
        ((element.flags & F.Object) !== 0 &&
          checker.getPropertiesOfType(element).length === 0 &&
          element.getCallSignatures().length === 0)
      );
    };

    const report = (node, messageId) => context.report({ node, messageId });

    const checkCall = (node) => {
      const name = calleeName(node.callee);
      const coercing =
        (node.callee.type === "Identifier" && COERCING_CALLEES.has(name)) ||
        (node.callee.type === "MemberExpression" && COERCING_MEMBERS.has(name));
      node.arguments.forEach((argument, index) => {
        const value =
          argument.type === "SpreadElement" ? argument.argument : argument;
        if (isMoney(value) && (coercing || openParameter(node, index))) {
          report(argument, "conversion");
        }
      });
    };

    const isDeclaredMember = (node) => {
      const symbol = checker.getSymbolAtLocation(
        services.esTreeNodeToTSNodeMap.get(node).name,
      );
      return symbol !== undefined && declaredIn(context, symbol, [moneyFile]);
    };

    const isKeyConstant = (node) => {
      const tsNode = services.esTreeNodeToTSNodeMap.get(node);
      const found = checker.getSymbolAtLocation(tsNode);
      const symbol =
        found !== undefined && (found.flags & ts.SymbolFlags.Alias) !== 0
          ? checker.getAliasedSymbol(found)
          : found;
      return (
        symbol !== undefined &&
        declaredIn(context, symbol, [moneyFile]) &&
        (checker.getTypeOfSymbol(symbol).flags & F.UniqueESSymbol) !== 0
      );
    };

    return {
      BinaryExpression: (node) => {
        const money = isMoney(node.left) || isMoney(node.right);
        const nullCheck =
          NULLISH_COMPARISON.has(node.operator) &&
          (isNullish(node.left) || isNullish(node.right));
        if (money && !nullCheck) report(node, "operator");
      },
      AssignmentExpression: (node) => {
        if (
          node.operator !== "=" &&
          (isMoney(node.left) || isMoney(node.right))
        )
          report(node, "operator");
      },
      UnaryExpression: (node) => {
        if (COERCING_UNARY.has(node.operator) && isMoney(node.argument))
          report(node, "operator");
      },
      UpdateExpression: (node) => {
        if (isMoney(node.argument)) report(node, "operator");
      },
      SwitchStatement: (node) => {
        if (isMoney(node.discriminant)) report(node, "operator");
      },
      TemplateLiteral: (node) => {
        if (node.expressions.some(isMoney)) report(node, "template");
      },
      CallExpression: checkCall,
      NewExpression: checkCall,
      MemberExpression: (node) => {
        if (!isMoney(node.object)) return;
        if (node.computed) {
          if (!readsKeys) report(node, "extraction");
        } else if (!isDeclaredMember(node)) {
          report(node, "member");
        }
      },
      ObjectPattern: (node) => {
        if (readsKeys || !isMoney(node)) return;
        node.properties
          .filter(
            (property) => property.type === "RestElement" || property.computed,
          )
          .forEach((property) => report(property, "extraction"));
      },
      Identifier: (node) => {
        if (!readsKeys && isKeyConstant(node)) report(node, "extraction");
      },
    };
  },
};
