import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");
const require = createRequire(join(root, "apps/desktop/package.json"));
const ts = require("typescript");
const output = join(root, "target/DS-20");
mkdirSync(output, { recursive: true });
const source = readFileSync(
  join(root, "apps/desktop/src/shared/ds/testing/overflow.ts"),
  "utf8",
);
const javascript = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2020,
  },
}).outputText;
const script = join(output, "overflow-wk.js");
writeFileSync(
  script,
  `var exports = {};\n${javascript}\nwindow.dsOverflow = exports;\n`,
);
const result = spawnSync(
  "swift",
  [join(root, "harness/ds-overflow/wk-overflow.swift"), script],
  {
    cwd: root,
    encoding: "utf8",
    timeout: 40000,
  },
);
if (result.status !== 0)
  throw new Error(`WKWebView 실측 실패: ${result.stderr}`);
const actual = JSON.parse(result.stdout);
const expected = {
  "document-scroll": ["document-scroll"],
  "missing-tooltip": ["missing-tooltip"],
  "valid-tooltip": [],
  "fixed-height": ["unauthorized-overflow"],
  flex: ["unauthorized-overflow"],
  "clipped-popup": ["clipped"],
  "nested-scroll": ["nested-scroll"],
  "outside-window": ["outside-window"],
  overlap: ["overlapping-controls"],
};
if (
  actual.length !== Object.keys(expected).length ||
  actual.some(
    (row) =>
      JSON.stringify([
        ...new Set(row.findings.map((finding) => finding.kind)),
      ]) !== JSON.stringify(expected[row.name]),
  )
)
  throw new Error(`WKWebView 판정 불일치: ${result.stdout}`);
writeFileSync(
  join(output, "wk-overflow.json"),
  `${JSON.stringify(actual, null, 2)}\n`,
);
process.stdout.write(`WKWebView: ${actual.length}개 예제 측정\n`);
