import { createRequire } from "node:module";
import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");
const require = createRequire(join(root, "apps/desktop/package.json"));
const ts = require("typescript");
const output = join(root, "target/DS-20");
mkdirSync(output, { recursive: true });
const compile = (file) =>
  ts.transpileModule(
    readFileSync(
      join(root, `apps/desktop/src/shared/ds/testing/${file}`),
      "utf8",
    ),
    {
      compilerOptions: {
        module: ts.ModuleKind.CommonJS,
        target: ts.ScriptTarget.ES2020,
      },
    },
  ).outputText;
const geometry = compile("overflow-geometry.ts");
const javascript = compile("overflow.ts");
const script = join(output, "overflow-wk.js");
writeFileSync(
  script,
  `var geometryExports = {};\n(function(exports){${geometry}\n})(geometryExports);\nvar exports = {};\nfunction require(name){if(name !== "./overflow-geometry") throw new Error("unexpected module");return geometryExports;}\n${javascript}\nwindow.dsOverflow = exports;\n`,
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
  "fixed-locked": [],
  "fixed-transformed": ["clipped"],
  "padding-edge": ["clipped"],
};
const geometryExpected = {
  "fixed-locked": true,
  "fixed-transformed": true,
  "padding-edge": true,
};
if (
  actual.length !== Object.keys(expected).length ||
  actual.some(
    (row) =>
      row.mode !== "CSS1Compat" ||
      row.geometry !== (geometryExpected[row.name] ?? null) ||
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
