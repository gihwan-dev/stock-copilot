import { basename } from "node:path";
import { projectPath } from "../types.mjs";

const SOURCE_EXTENSIONS = [".ts", ".tsx"];

const outsideFeatureLayout = (path, entries) => {
  const parts = path.split("/");
  return (
    entries !== undefined &&
    parts.length >= 4 &&
    parts[0] === "src" &&
    parts[1] === "features" &&
    !entries.includes(parts[3])
  );
};

const hasSourceExtension = (path) =>
  SOURCE_EXTENSIONS.some((extension) => path.endsWith(extension)) &&
  !path.endsWith(".d.ts");

export const forbiddenFile = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Report source files outside the per-folder file allowlists and files that are not .ts or .tsx",
    },
    schema: [
      {
        type: "object",
        properties: {
          checkExtension: { type: "boolean" },
          placeholders: { type: "array", items: { type: "string" } },
          folders: {
            type: "array",
            items: {
              type: "object",
              properties: {
                folder: { type: "string" },
                files: { type: "array", items: { type: "string" } },
              },
              required: ["folder", "files"],
              additionalProperties: false,
            },
          },
          featureEntries: { type: "array", items: { type: "string" } },
        },
        required: ["checkExtension", "placeholders", "folders"],
        additionalProperties: false,
      },
    ],
    messages: {
      extension:
        "src에는 소문자 확장자 .ts·.tsx 파일만 둔다. 다른 확장자, 대문자 확장자, .d.ts는 금지다.",
      unlisted:
        "허용 목록이 있는 폴더(shared/platform)에는 목록에 있는 파일만 둔다. 폴더로 옮겨 규칙을 피할 수 없다.",
      featureLayout:
        "features/<이름>/ 바로 아래에는 ui/, model/, index.ts, messages.ts만 둔다 (architecture 5장).",
    },
  },
  create: (context) => {
    const [{ checkExtension, placeholders, folders, featureEntries }] =
      context.options;
    return {
      Program: (node) => {
        const path = projectPath(context, context.filename);
        const placeholder =
          placeholders.includes(basename(path)) &&
          context.sourceCode.text === "";
        if (checkExtension && !placeholder && !hasSourceExtension(path)) {
          context.report({ node, messageId: "extension" });
        }
        folders
          .filter(
            ({ folder, files }) =>
              path.startsWith(`${folder}/`) && !files.includes(path),
          )
          .forEach(() => context.report({ node, messageId: "unlisted" }));
        if (outsideFeatureLayout(path, featureEntries)) {
          context.report({ node, messageId: "featureLayout" });
        }
      },
    };
  },
};

export const emptyParser = {
  meta: { name: "local/empty-parser" },
  parseForESLint: (text) => ({
    ast: {
      type: "Program",
      body: [],
      sourceType: "module",
      range: [0, text.length],
      loc: {
        start: { line: 1, column: 0 },
        end: { line: 1, column: 0 },
      },
      tokens: [],
      comments: [],
    },
  }),
};
