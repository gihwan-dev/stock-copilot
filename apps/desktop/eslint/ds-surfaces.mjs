import ts from "typescript";
export const INDEX_HTML = `<!doctype html>
<html lang="ko">
  <head>
    <meta charset="UTF-8" />
    <link rel="stylesheet" href="/src/shared/ds/theme.css" />
    <title>Stock Copilot</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/app/main.tsx"></script>
  </body>
</html>
`;
export const surfaceProblems = (html, vite) => {
  const problems = html === INDEX_HTML ? [] : ["index.html changed"];
  const ast = ts.createSourceFile(
    "vite.config.mjs",
    vite,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.JS,
  );
  const exports = ast.statements.filter(ts.isExportAssignment);
  const expression = exports[0]?.expression;
  if (
    exports.length !== 1 ||
    expression === undefined ||
    !ts.isCallExpression(expression) ||
    expression.expression.getText(ast) !== "defineConfig" ||
    expression.arguments.length !== 1 ||
    !ts.isObjectLiteralExpression(expression.arguments[0])
  )
    problems.push("Vite config must be a static object");
  const visit = (node) => {
    if (ts.isSpreadAssignment(node))
      problems.push("Vite config spread forbidden");
    if (ts.isPropertyAssignment(node)) {
      const key = node.name;
      const value = ts.isComputedPropertyName(key)
        ? ts.isStringLiteral(key.expression)
          ? key.expression.text
          : undefined
        : key.text;
      if (value === undefined || ["css", "postcss"].includes(value))
        problems.push("Vite CSS config forbidden");
    }
    ts.forEachChild(node, visit);
  };
  visit(ast);
  return problems;
};
