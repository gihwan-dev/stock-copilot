import assert from "node:assert/strict";
import test from "node:test";
import {
  readFileSync,
  writeFileSync,
  mkdirSync,
  rmSync,
  cpSync,
} from "node:fs";
import { join } from "node:path";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
const original = readFileSync(
  new URL("../eslint/ds-semantic-variants.mjs", import.meta.url),
  "utf8",
);
const publicSource = readFileSync(
  new URL("../eslint/ds-public-props.mjs", import.meta.url),
  "utf8",
);
const cases = JSON.parse(
  readFileSync(
    new URL(
      "../../../harness/eslint-violations/ds04-semantic.json",
      import.meta.url,
    ),
    "utf8",
  ),
);
const mutations = [
  [
    "map declaration location",
    "!dsFile(declaration)",
    "false",
    "outside DS map",
  ],
  [
    "map const",
    "(declaration.parent.flags & ts.NodeFlags.Const) === 0",
    "false",
    "mutable map",
  ],
  [
    "duplicate keys",
    "new Set(pairs.map((p) => p[0])).size !== pairs.length",
    "false",
    "duplicate map keys",
  ],
  [
    "reserved keys",
    '["__proto__", "constructor", "prototype"].includes(p[0])',
    "false",
    "reserved map key",
  ],
  [
    "declaration price permission",
    "!staticValue(p[1], object.getSourceFile())",
    "false",
    "price declaration permission",
  ],
  [
    "consumer price permission",
    "!staticValue(p[1], consumer)",
    "false",
    "price map consumer permission",
  ],
  [
    "variant class restriction",
    "parts.every((part) => variants.has(part))",
    "true",
    "invalid class variant",
  ],
  ["class restriction", "classNames.has(base)", "true", "unknown class"],
  [
    "complete map keys",
    "!values.every((value) => keys.includes(value))",
    "false",
    "missing member",
  ],
  [
    "closed source type",
    "!declarations.every((d) => closed(d.type, d.getSourceFile()))",
    "false",
    "template union",
  ],
  [
    "actual parameter",
    "!ts.isParameter(parameter) || ownerOf(parameter)?.parameters[0] !== parameter",
    "false",
    "typed const is not props",
  ],
  [
    "local const",
    "(d.parent.flags & ts.NodeFlags.Const)===0",
    "false",
    "mutable local alias",
  ],
  [
    "both class branches",
    "classes(node.whenTrue) && classes(node.whenFalse)",
    "classes(node.whenTrue) || classes(node.whenFalse)",
    "incomplete conditional",
  ],
  [
    "both consumption branches",
    "proof(value.whenTrue,property,owner)&&proof(value.whenFalse,property,owner)",
    "proof(value.whenTrue,property,owner)||proof(value.whenFalse,property,owner)",
    "narrowed conditional only one branch",
  ],
  [
    "switch arity",
    "fn.parameters.length !== 1",
    "false",
    "switch extra argument",
  ],
  [
    "switch literal classes",
    "!staticValue(c.statements[0].expression.text, consumer)",
    "false",
    "price switch consumer permission",
  ],
];
const data = (s) =>
  "data:text/javascript;base64," + Buffer.from(s).toString("base64");
const importRules = async (source) => {
  source = source
    .replace('"typescript"', JSON.stringify(import.meta.resolve("typescript")))
    .replace(
      '"./ds-classes.mjs"',
      JSON.stringify(new URL("../eslint/ds-classes.mjs", import.meta.url).href),
    );
  const semanticUrl = data(source);
  const semantic = await import(semanticUrl);
  let pub = publicSource
    .replace('"typescript"', JSON.stringify(import.meta.resolve("typescript")))
    .replace('"./ds-semantic-variants.mjs"', JSON.stringify(semanticUrl));
  for (const name of ["ds-boundaries", "allowlists"])
    pub = pub.replace(
      `"./${name}.mjs"`,
      JSON.stringify(new URL(`../eslint/${name}.mjs`, import.meta.url).href),
    );
  return {
    public: (await import(data(pub))).publicProps,
    static: {
      meta: { schema: [] },
      create(ctx) {
        return {
          JSXAttribute(node) {
            if (
              node.name.name !== "className" ||
              node.value?.type !== "JSXExpressionContainer"
            )
              return;
            const value =
              ctx.sourceCode.parserServices.esTreeNodeToTSNodeMap.get(
                node.value.expression,
              );
            if (!semantic.semanticVariants(ctx).classes(value))
              ctx.report({ node, message: "static contract" });
          },
        };
      },
    },
  };
};
const edit = (source, needle, replacement) => {
  const offsets = [];
  let compact = "";
  for (let i = 0; i < source.length; i++)
    if (!/\s/u.test(source[i])) {
      compact += source[i];
      offsets.push(i);
    }
  const target = needle.replace(/\s/gu, "");
  const start = compact.indexOf(target);
  assert.ok(start >= 0, needle + " missing target");
  return (
    source.slice(0, offsets[start]) +
    replacement +
    source.slice(offsets[start + target.length - 1] + 1)
  );
};
test("DS04 sixteen semantic guard mutations lose a regression witness", async () => {
  const folder = mkdtempSync("ds04-semantic-mutations-");
  try {
    cpSync(join(desktopRoot, "src"), join(folder, "src"), { recursive: true });
    const project = join(folder, "tsconfig.json"),
      source = join(folder, "src/shared/ds/Probe.tsx");
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
    const base = new ESLint({
      cwd: folder,
      overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
    });
    const config = await base.calculateConfigForFile(source);
    const engine = async (code) =>
      new ESLint({
        cwd: folder,
        overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
        overrideConfig: [
          {
            settings: { "ds/price-color-files": ["src/shared/ds/Badge.tsx"] },
            languageOptions: {
              parserOptions: {
                projectService: false,
                project,
                tsconfigRootDir: folder,
              },
            },
            plugins: { witness: { rules: await importRules(code) } },
            rules: {
              ...Object.fromEntries(
                Object.keys(config.rules).map((n) => [n, "off"]),
              ),
              "witness/public": "error",
              "witness/static": "error",
            },
          },
        ],
      });
    for (const [name, needle, replacement, fixture] of mutations) {
      const entry = cases.find((c) => c.name === fixture);
      assert.ok(entry, fixture);
      for (const [file, code] of Object.entries(entry.files ?? {})) {
        const path = join(folder, "src/shared/ds", file);
        mkdirSync(join(path, ".."), { recursive: true });
        writeFileSync(path, code);
      }
      for (const [file, code] of Object.entries(entry.appendFiles ?? {}))
        writeFileSync(
          join(folder, "src/shared/ds", file),
          readFileSync(join(desktopRoot, "src/shared/ds", file), "utf8") + code,
        );
      const target = entry.sourcePath
        ? join(folder, "src/shared/ds", entry.sourcePath)
        : source;
      mkdirSync(join(target, ".."), { recursive: true });
      writeFileSync(
        target,
        'import type {ReactElement} from "react";' + entry.code,
      );
      const beforeEngine = await engine(original),
        afterEngine = await engine(edit(original, needle, replacement));
      for (const file of Object.keys({
        ...entry.files,
        ...entry.appendFiles,
      })) {
        await beforeEngine.lintFiles([join(folder, "src/shared/ds", file)]);
        await afterEngine.lintFiles([join(folder, "src/shared/ds", file)]);
      }
      const before = (await beforeEngine.lintFiles([target]))[0].messages,
        after = (await afterEngine.lintFiles([target]))[0].messages;
      assert.ok(before.length > 0, name + " no baseline rejection");
      assert.ok(
        after.length < before.length,
        name + " survived " + JSON.stringify({ before, after }),
      );
      process.stdout.write("DS04 killed mutation: " + name + "\n");
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
