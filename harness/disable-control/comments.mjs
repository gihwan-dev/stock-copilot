import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { dirname, extname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const ts = createRequire(
  join(here, "..", "..", "apps", "desktop", "package.json"),
)("typescript");

const kinds = {
  ".ts": ts.ScriptKind.TS,
  ".mts": ts.ScriptKind.TS,
  ".cts": ts.ScriptKind.TS,
  ".tsx": ts.ScriptKind.TSX,
  ".js": ts.ScriptKind.JS,
  ".mjs": ts.ScriptKind.JS,
  ".cjs": ts.ScriptKind.JS,
  ".jsx": ts.ScriptKind.JSX,
};

const escaped = (text) =>
  text
    .replaceAll("\\", "\\\\")
    .replaceAll("\n", "\\n")
    .replaceAll("\r", "\\r")
    .replaceAll("\t", "\\t");

const commentsOf = (name, text) => {
  const source = ts.createSourceFile(
    name,
    text,
    ts.ScriptTarget.Latest,
    true,
    kinds[extname(name)] ?? ts.ScriptKind.TS,
  );
  const ranges = new Map();
  const jsxText = [];
  const add = (found) =>
    (found ?? []).forEach((range) => ranges.set(range.pos, range));
  const visit = (node) => {
    if (node.kind === ts.SyntaxKind.JsxText) {
      jsxText.push([node.pos, node.end]);
    }
    add(ts.getLeadingCommentRanges(text, node.pos));
    add(ts.getTrailingCommentRanges(text, node.end));
    node.getChildren(source).forEach(visit);
  };
  visit(source);
  const insideJsxText = (position) =>
    jsxText.some(([start, end]) => position >= start && position < end);
  return [...ranges.values()]
    .filter((range) => !insideJsxText(range.pos))
    .sort((left, right) => left.pos - right.pos)
    .map((range) => {
      const raw = text.slice(range.pos, range.end);
      const body =
        range.kind === ts.SyntaxKind.SingleLineCommentTrivia
          ? raw.slice(2)
          : raw.slice(2, -2);
      const start = source.getLineAndCharacterOfPosition(range.pos).line + 1;
      const end = source.getLineAndCharacterOfPosition(range.end).line + 1;
      return `${start}\t${end}\t${escaped(body)}`;
    });
};

const files = JSON.parse(readFileSync(0, "utf8"));
const lines = files.flatMap(([name, text], index) =>
  commentsOf(name, text).map((line) => `${index}\t${line}`),
);
process.stdout.write(lines.map((line) => `${line}\n`).join(""));
