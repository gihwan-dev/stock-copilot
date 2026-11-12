import { projectPath } from "../types.mjs";

const testFile = /\.test\.tsx?$/;
const testingFolder = "src/shared/ipc/testing/";
const testingSource = /(^|\/)testing(\/|$)/;

const sourceOf = (node) =>
  node.type === "TSImportType"
    ? node.argument?.literal?.value
    : node.type === "TSExternalModuleReference"
      ? node.expression?.value
      : node.source?.value;

export const ipcTestingBoundary = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Allow imports of shared/ipc/testing only from test files and from inside the testing folder",
    },
    schema: [],
    messages: {
      testingImport:
        "shared/ipc/testing은 테스트 파일(*.test.ts·*.test.tsx)과 testing 폴더 안에서만 가져온다.",
    },
  },
  create: (context) => {
    const path = projectPath(context, context.filename);
    const allowed = testFile.test(path) || path.startsWith(testingFolder);
    const checkSource = (node) => {
      const source = sourceOf(node);
      if (!allowed && typeof source === "string" && testingSource.test(source))
        context.report({ node, messageId: "testingImport" });
    };
    return {
      ImportDeclaration: checkSource,
      ExportNamedDeclaration: checkSource,
      ExportAllDeclaration: checkSource,
      ImportExpression: checkSource,
      TSImportType: checkSource,
      TSExternalModuleReference: checkSource,
    };
  },
};
