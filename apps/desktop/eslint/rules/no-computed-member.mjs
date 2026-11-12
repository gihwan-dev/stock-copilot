import ts from "typescript";
import { isProjectObject, typeFlags as F } from "../types.mjs";

const isLiteralKey = (node) =>
  (node.type === "Literal" &&
    (typeof node.value === "string" || typeof node.value === "number")) ||
  (node.type === "TemplateLiteral" && node.expressions.length === 0);

const isLiteralUnion = (type) =>
  type.isUnion()
    ? type.types.every(isLiteralUnion)
    : (type.flags & (F.StringLiteral | F.NumberLiteral)) !== 0;

const partsOf = (type) => (type.isUnion() ? type.types : [type]);

export const noComputedMember = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Allow obj[expr] with a non-literal key only for literal-union keys on project-declared objects",
    },
    schema: [],
    messages: {
      key: "계산된 키는 리터럴이거나 리터럴 유니온 타입이어야 한다. symbol·타입 매개변수·문자열 키로 멤버를 고르지 않는다.",
      target:
        "계산된 키로 고르는 대상은 프로젝트에서 선언한 객체여야 한다. 네임스페이스·라이브러리 객체는 이름으로 접근한다.",
      index: "배열·튜플은 리터럴 숫자 색인만 쓴다. 그 밖에는 .at() 등을 쓴다.",
    },
  },
  create: (context) => {
    const services = context.sourceCode.parserServices;
    const checker = services.program.getTypeChecker();
    const report = (node, messageId) => context.report({ node, messageId });

    const isModule = (type) =>
      ((type.getSymbol()?.flags ?? 0) & ts.SymbolFlags.ValueModule) !== 0;

    const isList = (type) =>
      checker.isArrayType(type) || checker.isTupleType(type);

    const check = (key, objectType, at) => {
      if (isLiteralKey(key)) return;
      const parts = partsOf(checker.getNonNullableType(objectType));
      if (parts.some(isList)) return report(at, "index");
      if (!isLiteralUnion(services.getTypeAtLocation(key)))
        return report(at, "key");
      if (parts.some((part) => isModule(part) || !isProjectObject(part)))
        return report(at, "target");
      return undefined;
    };

    return {
      "MemberExpression[computed=true]": (node) =>
        check(node.property, services.getTypeAtLocation(node.object), node),
      "ObjectPattern > Property[computed=true]": (node) =>
        check(node.key, services.getTypeAtLocation(node.parent), node),
    };
  },
};
