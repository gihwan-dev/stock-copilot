import ts from "typescript";
import { projectPath } from "../types.mjs";
export const numberDisplayBoundary = {
  meta: {
    type: "problem",
    schema: [],
    messages: {
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
        if (mayRead) return;
        const symbol = checker.getSymbolAtLocation(
          services.esTreeNodeToTSNodeMap.get(node),
        );
        const resolved =
          symbol && (symbol.flags & ts.SymbolFlags.Alias) !== 0
            ? checker.getAliasedSymbol(symbol)
            : symbol;
        if (
          (resolved?.declarations ?? []).some(
            (d) =>
              d.name?.getText() === "readDecimalForNumberDisplay" &&
              /\/src\/shared\/lib\/money\.ts$/u.test(
                d.getSourceFile().fileName,
              ),
          )
        )
          context.report({ node, messageId: "reader" });
      },
    };
  },
};
