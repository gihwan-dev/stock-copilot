import ts from "typescript";
import { DS_CLASSES, DS_VARIANTS } from "./ds-classes.mjs";
const cache = new WeakMap();
const dsFile = (node) =>
  /\/src\/shared\/ds\/.+\.tsx?$/u.test(node.getSourceFile().fileName);
const generated = (node) =>
  /\/src\/shared\/ds\/tokens\.ts$/u.test(node.getSourceFile().fileName);
export const semanticName = (name) =>
  /^(tone|variant|severity|status)$|(?:Tone|Variant|Severity|Status)$|_(tone|variant|severity|status)$/u.test(
    name,
  );
const unwrap = (node) => {
  while (
    node !== undefined &&
    (ts.isParenthesizedExpression(node) ||
      ts.isAsExpression(node) ||
      ts.isSatisfiesExpression(node))
  )
    node = node.expression;
  return node;
};
export const semanticVariants = (ctx) => {
  const program = ctx.sourceCode.parserServices.program;
  if (program === undefined || program === null)
    return {
      contract: () => false,
      tokenOnly: () => false,
      classes: () => false,
    };
  const checker = program.getTypeChecker();
  const priceFiles = ctx.settings["ds/price-color-files"] ?? [];
  const key = JSON.stringify(priceFiles);
  const entries = cache.get(program) ?? new Map();
  cache.set(program, entries);
  if (entries.has(key)) return entries.get(key);
  const symbol = (node) => {
    const value = checker.getSymbolAtLocation(node);
    return value && (value.flags & ts.SymbolFlags.Alias) !== 0
      ? checker.getAliasedSymbol(value)
      : value;
  };
  const aliases = (node) =>
    (symbol(node.typeName)?.declarations ?? []).filter(
      ts.isTypeAliasDeclaration,
    );
  const literals = (type) => {
    const values = (type.isUnion() ? type.types : [type]).filter(
      (t) => (t.flags & (ts.TypeFlags.Undefined | ts.TypeFlags.Null)) === 0,
    );
    return values.length > 0 &&
      values.every((t) => (t.flags & ts.TypeFlags.StringLiteral) !== 0)
      ? values.map((t) => t.value)
      : undefined;
  };
  const closed = (node, owner, imported = true, seen = new Set()) => {
    if (
      node === undefined ||
      seen.has(node) ||
      seen.size > 32 ||
      !dsFile(node) ||
      generated(node)
    )
      return false;
    seen.add(node);
    if (ts.isLiteralTypeNode(node)) return ts.isStringLiteral(node.literal);
    if (ts.isUnionTypeNode(node))
      return node.types.every((t) =>
        closed(t, node.getSourceFile(), false, new Set(seen)),
      );
    if (!ts.isTypeReferenceNode(node) || node.typeArguments !== undefined)
      return false;
    const declarations = aliases(node);
    return (
      declarations.length > 0 &&
      declarations.every(
        (d) =>
          (imported || d.getSourceFile() === owner) &&
          closed(
            d.type,
            imported ? d.getSourceFile() : owner,
            imported,
            new Set(seen),
          ),
      )
    );
  };
  const tokenOnly = (node, seen = new Set()) => {
    if (node === undefined || seen.has(node) || seen.size > 32) return false;
    if (generated(node)) return true;
    seen.add(node);
    if (!ts.isTypeReferenceNode(node)) return false;
    if (
      ["Extract", "Exclude"].includes(node.typeName.getText()) &&
      node.typeArguments?.length === 2
    )
      return (
        tokenOnly(node.typeArguments[0], seen) &&
        closed(node.typeArguments[1], node.getSourceFile())
      );
    return aliases(node).some((d) => tokenOnly(d.type, new Set(seen)));
  };
  const classNames = new Set(DS_CLASSES),
    variants = new Set(DS_VARIANTS);
  const staticValue = (value, source) =>
    value.trim().length > 0 &&
    value
      .trim()
      .split(/\s+/u)
      .every((entry) => {
        const parts = entry.split(":");
        const base = parts.pop();
        return (
          parts.every((part) => variants.has(part)) &&
          classNames.has(base) &&
          (!/^(text|bg|border|fill|stroke|outline|ring|decoration)-(up|down|on-up|on-down)(-|$)/u.test(
            base,
          ) ||
            priceFiles.some((file) => source.fileName.endsWith("/" + file)))
        );
      });
  const constantObject = (expression, consumer) => {
    const target = unwrap(expression);
    if (!ts.isIdentifier(target)) return undefined;
    const declaration = (symbol(target)?.declarations ?? []).find(
      ts.isVariableDeclaration,
    );
    if (
      declaration === undefined ||
      !dsFile(declaration) ||
      !ts.isVariableDeclarationList(declaration.parent) ||
      (declaration.parent.flags & ts.NodeFlags.Const) === 0
    )
      return undefined;
    const object = unwrap(declaration.initializer);
    if (object === undefined || !ts.isObjectLiteralExpression(object))
      return undefined;
    const pairs = object.properties.map((p) =>
      ts.isPropertyAssignment(p) &&
      (ts.isIdentifier(p.name) || ts.isStringLiteral(p.name)) &&
      ts.isStringLiteral(p.initializer)
        ? [p.name.text, p.initializer.text]
        : undefined,
    );
    if (
      pairs.some((p) => p === undefined) ||
      new Set(pairs.map((p) => p[0])).size !== pairs.length ||
      pairs.some(
        (p) =>
          ["__proto__", "constructor", "prototype"].includes(p[0]) ||
          !staticValue(p[1], object.getSourceFile()) ||
          !staticValue(p[1], consumer),
      )
    )
      return undefined;
    return pairs.map((p) => p[0]);
  };
  const propertyOf = (expression, seen = new Set()) => {
    const node = unwrap(expression);
    if (node === undefined || seen.has(node) || seen.size > 32)
      return undefined;
    seen.add(node);
    if (ts.isIdentifier(node)) {
      const declaration = (symbol(node)?.declarations ?? []).find(
        ts.isVariableDeclaration,
      );
      if (
        declaration === undefined ||
        !dsFile(declaration) ||
        !ts.isVariableDeclarationList(declaration.parent) ||
        (declaration.parent.flags & ts.NodeFlags.Const) === 0
      )
        return undefined;
      return propertyOf(declaration.initializer, seen);
    }
    if (
      ts.isBinaryExpression(node) &&
      node.operatorToken.kind === ts.SyntaxKind.QuestionQuestionToken
    )
      return propertyOf(node.left, seen);
    return ts.isPropertyAccessExpression(node) && semanticName(node.name.text)
      ? symbol(node.name)
      : undefined;
  };
  const functionKeys = (expression, consumer) => {
    const declaration = (symbol(expression)?.declarations ?? []).find(
      (d) => ts.isFunctionDeclaration(d) || ts.isVariableDeclaration(d),
    );
    const fn =
      declaration !== undefined && ts.isVariableDeclaration(declaration)
        ? unwrap(declaration.initializer)
        : declaration;
    if (
      fn === undefined ||
      !dsFile(fn) ||
      !(
        ts.isFunctionDeclaration(fn) ||
        ts.isArrowFunction(fn) ||
        ts.isFunctionExpression(fn)
      ) ||
      fn.parameters.length !== 1 ||
      !ts.isBlock(fn.body) ||
      fn.body.statements.length !== 1
    )
      return undefined;
    const sw = fn.body.statements[0];
    if (
      !ts.isSwitchStatement(sw) ||
      symbol(sw.expression) !== symbol(fn.parameters[0].name)
    )
      return undefined;
    const expected = literals(checker.getTypeAtLocation(fn.parameters[0]));
    if (
      expected === undefined ||
      !closed(fn.parameters[0].type, fn.getSourceFile())
    )
      return undefined;
    const cases = sw.caseBlock.clauses;
    if (
      cases.some(
        (c) =>
          !ts.isCaseClause(c) ||
          !ts.isStringLiteral(c.expression) ||
          c.statements.length !== 1 ||
          !ts.isReturnStatement(c.statements[0]) ||
          c.statements[0].expression === undefined ||
          !ts.isStringLiteral(c.statements[0].expression) ||
          !staticValue(c.statements[0].expression.text, fn.getSourceFile()) ||
          !staticValue(c.statements[0].expression.text, consumer),
      )
    )
      return undefined;
    const keys = cases.map((c) => c.expression.text);
    return keys.length === expected.length &&
      new Set(keys).size === keys.length &&
      expected.every((value) => keys.includes(value))
      ? keys
      : undefined;
  };
  const mappings = [];
  const evaluate = (expression) => {
    const node = unwrap(expression);
    if (node === undefined) return undefined;
    const indexed = ts.isElementAccessExpression(node);
    const called = ts.isCallExpression(node) && node.arguments.length === 1;
    if (!indexed && !called) return undefined;
    const argument = indexed ? node.argumentExpression : node.arguments[0];
    const property = propertyOf(argument);
    if (property === undefined) return undefined;
    const keys = indexed
      ? constantObject(node.expression, node.getSourceFile())
      : functionKeys(node.expression, node.getSourceFile());
    const values = literals(checker.getTypeAtLocation(argument));
    const declarations = (property.declarations ?? []).filter(
      (d) => d.type !== undefined,
    );
    if (
      keys === undefined ||
      values === undefined ||
      !values.every((value) => keys.includes(value)) ||
      declarations.length === 0 ||
      !declarations.every((d) => closed(d.type, d.getSourceFile()))
    )
      return undefined;
    return { declarations, keys, source: node.getSourceFile() };
  };
  const classes = (node) => {
    if (node === undefined) return false;
    if (ts.isConditionalExpression(node))
      return classes(node.whenTrue) && classes(node.whenFalse);
    if (ts.isStringLiteral(node))
      return staticValue(node.text, node.getSourceFile());
    return evaluate(node) !== undefined;
  };
  for (const source of program.getSourceFiles()) {
    if (!dsFile(source)) continue;
    const visit = (node) => {
      if (
        ts.isJsxAttribute(node) &&
        node.name.getText() === "className" &&
        node.initializer !== undefined &&
        ts.isJsxExpression(node.initializer)
      ) {
        const expression = node.initializer.expression;
        const collect = (value) => {
          if (value === undefined) return;
          if (ts.isConditionalExpression(value)) {
            collect(value.whenTrue);
            collect(value.whenFalse);
            return;
          }
          const mapping = evaluate(value);
          if (mapping !== undefined) mappings.push(mapping);
        };
        collect(expression);
      }
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  const result = {
    classes,
    tokenOnly,
    contract: (property) => {
      const values = literals(checker.getTypeOfSymbol(property));
      const declarations = property.declarations ?? [];
      return (
        values !== undefined &&
        declarations.length > 0 &&
        declarations.every((d) => closed(d.type, d.getSourceFile())) &&
        mappings.some(
          (mapping) =>
            declarations.some(
              (d) =>
                mapping.declarations.includes(d) ||
                d.getSourceFile() === mapping.source,
            ) && values.every((value) => mapping.keys.includes(value)),
        )
      );
    },
  };
  entries.set(key, result);
  return result;
};
