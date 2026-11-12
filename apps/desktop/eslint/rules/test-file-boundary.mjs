const testFile = /\.test\.tsx?$/;
const testSource = /\.test(?:\.tsx?)?$/;

const sourceOf = (node) =>
  node.type === "TSImportType"
    ? node.argument?.literal?.value
    : node.type === "TSExternalModuleReference"
      ? node.expression?.value
      : node.source?.value;

export const testFileBoundary = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Keep test files out of the production module graph: no exports from test files and no imports of test files",
    },
    schema: [],
    messages: {
      testExport:
        "테스트 파일(*.test.ts·*.test.tsx)은 아무것도 내보낼 수 없다. 운영 코드는 테스트 파일 밖에 둔다.",
      testImport:
        "테스트 파일(*.test.ts·*.test.tsx)은 가져올 수 없다. 테스트 파일은 운영 코드의 통로가 될 수 없다.",
    },
  },
  create: (context) => {
    const isTest = testFile.test(context.filename);
    const reportExport = (node) => {
      if (isTest) context.report({ node, messageId: "testExport" });
    };
    const checkSource = (node) => {
      const source = sourceOf(node);
      if (typeof source === "string" && testSource.test(source))
        context.report({ node, messageId: "testImport" });
    };
    return {
      ExportNamedDeclaration: (node) => {
        reportExport(node);
        checkSource(node);
      },
      ExportAllDeclaration: (node) => {
        reportExport(node);
        checkSource(node);
      },
      ExportDefaultDeclaration: reportExport,
      TSExportAssignment: reportExport,
      ImportDeclaration: checkSource,
      ImportExpression: checkSource,
      TSImportType: checkSource,
      TSExternalModuleReference: checkSource,
    };
  },
};
