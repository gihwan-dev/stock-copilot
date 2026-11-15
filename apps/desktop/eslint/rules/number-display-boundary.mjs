import ts from "typescript";
import { projectPath, declaredIn } from "../types.mjs";
export const numberDisplayBoundary = {
  meta: {
    type: "problem",
    schema: [],
    messages: {
      formatter:
        "숫자 형식화 함수와 타입은 shared/ds/Numbers.tsx 내부에서만 쓴다.",
      reader:
        "표시용 십진수 읽기는 money.ts와 format.ts만 사용한다. 화면은 DS 숫자 컴포넌트를 쓴다.",
      conversion:
        "숫자 표시는 부동소수점 변환과 Intl 반올림 없이 문자열로 처리한다.",
    },
  },
  create(context) {
    const here = projectPath(context, context.filename);
    const services = context.sourceCode.parserServices;
    const checker = services.program.getTypeChecker();
    const mayRead = [
      "src/shared/lib/money.ts",
      "src/shared/lib/format.ts",
    ].includes(here);
    const display = here === "src/shared/lib/format.ts";
    return {
      Identifier(node) {
        if (
          display &&
          ["Number", "parseFloat", "parseInt", "Intl"].includes(node.name)
        )
          context.report({ node, messageId: "conversion" });

        const symbol = checker.getSymbolAtLocation(
          services.esTreeNodeToTSNodeMap.get(node),
        );
        const resolved =
          symbol && (symbol.flags & ts.SymbolFlags.Alias) !== 0
            ? checker.getAliasedSymbol(symbol)
            : symbol;
        const formatting = declaredIn(context, resolved, [
          "src/shared/lib/format.ts",
        ]);
        const exporting = node.parent.type === "ExportSpecifier";
        if (
          formatting &&
          (exporting ||
            !["src/shared/lib/format.ts", "src/shared/ds/Numbers.tsx"].includes(
              here,
            ))
        )
          context.report({ node, messageId: "formatter" });
        if (mayRead) return;
        if (
          (resolved?.declarations ?? []).some(
            (d) =>
              d.name?.getText() === "readDecimalForNumberDisplay" &&
              projectPath(context, d.getSourceFile().fileName) ===
                "src/shared/lib/money.ts",
          )
        )
          context.report({ node, messageId: "reader" });
      },
    };
  },
};
