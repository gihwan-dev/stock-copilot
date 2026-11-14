import assert from "node:assert/strict";
import test from "node:test";
import { mkdirSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import ts from "typescript";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";

const original = readFileSync(
  new URL("../eslint/ds-public-props.mjs", import.meta.url),
  "utf8",
);
const cases = JSON.parse(
  readFileSync(
    new URL(
      "../../../harness/eslint-violations/ds10-public.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
const mutations = [
  ["React event source", "React event branch", "if", "/Event$/u.test"],
  [
    "forwardRef call",
    "forwardRef branch",
    "if",
    'propertyName(node.callee) === "forwardRef"',
  ],
  [
    "nested index",
    "nested index",
    "if",
    "checker.getIndexInfosOfType(type).length !== 0",
    "badValue",
  ],
  ["DOM source", "DOM node", "if", "lib\\.dom"],
  [
    "generic constraint",
    "generic event constraint",
    "if",
    "ts.TypeFlags.TypeParameter",
    "badValue",
  ],
  ["callback return", "return DOM", "call", "badValue(s.getReturnType()"],
  [
    "array elements",
    "array event",
    "if",
    "checker.isArrayType(type) || checker.isTupleType(type)",
    "badValue",
  ],
  ["compound member", "compound", "call", ".some((p) =>\n        badExport"],
  [
    "generic props root",
    "generic entire",
    "if",
    "ts.TypeFlags.TypeParameter",
    "badProps",
  ],
  [
    "token category",
    "wrong token category",
    "binary",
    "category !== undefined",
  ],
  ["nested token", "nested gap", "if", "tokenProperty.test(p.name)"],
  ["ref identity", "fake ref", "if", 'p.name === "ref"'],
  ["broad number", "broad number", "property", "ts.TypeFlags.Number"],
  [
    "token origin",
    "copied token",
    "call",
    "generatedReference(checker, d.type)",
  ],
  ["JSX contextual number", "chart numbers", "call", "badValue(contextual)"],
];
const enclosing = (node) => {
  for (let parent = node.parent; parent !== undefined; parent = parent.parent)
    if (ts.isVariableDeclaration(parent)) return parent.name.getText();
  return undefined;
};
const edit = (spec) => {
  const [, , kind, needle, owner] = spec;
  const tree = ts.createSourceFile(
    "rule.mjs",
    original,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.JS,
  );
  const matches = [];
  const compact = (s) => s.replace(/\s+/gu, "");
  const visit = (node) => {
    const candidate =
      kind === "if"
        ? ts.isIfStatement(node)
        : kind === "call"
          ? ts.isCallExpression(node)
          : kind === "binary"
            ? ts.isBinaryExpression(node)
            : ts.isPropertyAccessExpression(node);
    const source =
      kind === "if" && ts.isIfStatement(node)
        ? node.expression.getText()
        : node.getText();
    if (
      candidate &&
      compact(source).includes(compact(needle)) &&
      (owner === undefined || enclosing(node) === owner)
    )
      matches.push(node);
    ts.forEachChild(node, visit);
  };
  visit(tree);
  matches.sort((a, b) => a.end - a.getStart() - (b.end - b.getStart()));
  assert.ok(matches.length > 0, spec[0] + " mutation target missing");
  const target = matches[0];
  return (
    original.slice(0, target.getStart()) +
    (kind === "if"
      ? ""
      : kind === "property"
        ? "0"
        : spec[0] === "token origin"
          ? "true"
          : "false") +
    original.slice(target.end)
  );
};
const importRule = async (source) => {
  source = source.replace(
    '"typescript"',
    JSON.stringify(import.meta.resolve("typescript")),
  );
  for (const name of ["ds-boundaries", "allowlists"])
    source = source.replace(
      `"./${name}.mjs"`,
      JSON.stringify(new URL(`../eslint/${name}.mjs`, import.meta.url).href),
    );
  return (
    await import(
      "data:text/javascript;base64," + Buffer.from(source).toString("base64")
    )
  ).publicProps;
};
test("DS10 fifteen independent branch deletions lose their sole witness", async () => {
  const folder = mkdtempSync("ds10-mutations-");
  const source = join(folder, "src/shared/ds/Probe.tsx"),
    project = join(folder, "tsconfig.json");
  mkdirSync(join(source, ".."), { recursive: true });
  writeFileSync(
    project,
    JSON.stringify({
      extends: join(desktopRoot, "tsconfig.json"),
      compilerOptions: {
        rootDir: "src",
        noUnusedLocals: false,
        noUnusedParameters: false,
      },
      include: ["src"],
    }),
  );
  writeFileSync(
    join(folder, "src/shared/ds/tokens.ts"),
    readFileSync(join(desktopRoot, "src/shared/ds/tokens.ts")),
  );
  const base = new ESLint({
    cwd: folder,
    overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
  });
  const config = await base.calculateConfigForFile(source);
  const rules = Object.fromEntries(
    Object.keys(config.rules).map((name) => [name, "off"]),
  );
  const engine = (rule) =>
    new ESLint({
      cwd: folder,
      overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
      overrideConfig: [
        {
          languageOptions: {
            parserOptions: {
              projectService: false,
              project,
              tsconfigRootDir: folder,
            },
          },
          plugins: { mutation: { rules: { contract: rule } } },
          rules: { ...rules, "mutation/contract": "error" },
        },
      ],
    });
  const baseline = engine(await importRule(original));
  try {
    for (const spec of mutations) {
      const code = cases.find(([name]) => name === spec[1])?.[1];
      assert.ok(code, spec[1]);
      writeFileSync(
        source,
        'import type {MouseEvent,ComponentProps} from "react";import type {SpacingToken} from "./tokens";type Text=string & {readonly text:unique symbol};' +
          code,
      );
      const before = (await baseline.lintFiles([source]))[0].messages;
      assert.ok(
        before.some((m) => m.ruleId === "mutation/contract"),
        spec[0] + JSON.stringify(before),
      );
      const after = (
        await engine(await importRule(edit(spec))).lintFiles([source])
      )[0].messages;
      assert.deepEqual(after, [], spec[0] + " survived");
      process.stdout.write(`DS10 killed mutation: ${spec[0]}\n`);
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
