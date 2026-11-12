import ts from "typescript";
import { rawTypeChecker } from "../types.mjs";

const RAW_KEYWORDS = [
  "TSStringKeyword",
  "TSNumberKeyword",
  "TSBigIntKeyword",
  "TSBooleanKeyword",
  "TSUnknownKeyword",
  "TSAnyKeyword",
  "TSObjectKeyword",
];
const RAW_REFERENCES = new Set([
  "String",
  "Number",
  "BigInt",
  "Boolean",
  "Object",
]);
const RESOLVED_NODES = [
  "TSTypeReference",
  "TSTypeQuery",
  "TSTypeOperator",
  "TSIndexedAccessType",
  "TSConditionalType",
  "TSMappedType",
  "TSImportType",
  "TSTemplateLiteralType",
  "TSTypeLiteral",
];
const JSX_RESULT = /^(React\.)?(JSX\.Element|ReactElement|ReactNode)\b/u;

const FUNCTION_NODES = [
  "ArrowFunctionExpression",
  "FunctionExpression",
  "FunctionDeclaration",
];

export const noRawTypes = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Disallow string, number, bigint, boolean and open types in type positions, written or resolved",
    },
    schema: [],
    messages: {
      written:
        "타입 자리에 string·number·bigint·boolean·true·false·unknown·any·object·{}를 쓰지 않는다. shared/lib/brand의 브랜드나 리터럴 유니온을 쓴다.",
      resolved:
        "이 타입은 풀면 원시 타입이나 열린 타입이 된다(별칭, typeof, keyof, 템플릿 리터럴, 매핑·조건부 타입 포함). 브랜드나 리터럴 유니온을 쓴다.",
      exported:
        "내보내는 값의 타입(함수 매개변수·반환 타입 포함)에 원시 타입이 남아 있다.",
      props: "컴포넌트 props에 원시 타입이 남아 있다.",
    },
  },
  create: (context) => {
    const services = context.sourceCode.parserServices;
    const checker = services.program.getTypeChecker();
    const { isRaw, signatureRaw } = rawTypeChecker(checker);
    const tsNodeOf = (node) => services.esTreeNodeToTSNodeMap.get(node);

    const checkResolved = (node) => {
      if (isRaw(checker.getTypeFromTypeNode(tsNodeOf(node))))
        context.report({ node, messageId: "resolved" });
    };

    const checkReference = (node) => {
      const name =
        node.typeName.type === "Identifier" ? node.typeName.name : undefined;
      if (name === "const") return;
      if (RAW_REFERENCES.has(name))
        context.report({ node, messageId: "written" });
      else checkResolved(node);
    };

    const checkExports = (program) => {
      const module = checker.getSymbolAtLocation(tsNodeOf(program));
      if (module === undefined) return;
      checker.getExportsOfModule(module).forEach((exported) => {
        const symbol =
          (exported.flags & ts.SymbolFlags.Alias) !== 0
            ? checker.getAliasedSymbol(exported)
            : exported;
        const declaration = symbol.valueDeclaration;
        if (
          declaration === undefined ||
          declaration.getSourceFile() !== tsNodeOf(program)
        )
          return;
        const type = checker.getTypeOfSymbolAtLocation(symbol, declaration);
        const signatures = type.getCallSignatures();
        const raw =
          signatures.length > 0 ? signatures.some(signatureRaw) : isRaw(type);
        if (raw) {
          const local = exported.declarations?.[0];
          const node =
            (local === undefined
              ? undefined
              : services.tsNodeToESTreeNodeMap.get(local)) ?? program;
          context.report({ node, messageId: "exported" });
        }
      });
    };

    const checkComponent = (node) => {
      const signature = checker.getSignatureFromDeclaration(tsNodeOf(node));
      if (signature === undefined) return;
      const result = checker.typeToString(signature.getReturnType());
      if (!JSX_RESULT.test(result)) return;
      node.params
        .filter((param) => isRaw(services.getTypeAtLocation(param)))
        .forEach((param) =>
          context.report({ node: param, messageId: "props" }),
        );
    };

    return {
      [RAW_KEYWORDS.join(", ")]: (node) =>
        context.report({ node, messageId: "written" }),
      TSLiteralType: (node) => {
        if (
          node.literal.type === "Literal" &&
          typeof node.literal.value === "boolean"
        )
          context.report({ node, messageId: "written" });
      },
      TSTypeReference: checkReference,
      [RESOLVED_NODES.filter((name) => name !== "TSTypeReference").join(", ")]:
        checkResolved,
      [FUNCTION_NODES.join(", ")]: checkComponent,
      "Program:exit": checkExports,
    };
  },
};
