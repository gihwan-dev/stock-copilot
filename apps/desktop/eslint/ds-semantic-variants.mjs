import { overflowRestrictions, overflowKeywords } from "./ds-overflow.mjs";
import ts from "typescript";
import { relative, dirname } from "node:path";
import { DS_CLASSES, DS_VARIANTS } from "./ds-classes.mjs";
const cache = new WeakMap();
export const semanticName = (name) =>
  [
    "tone",
    "variant",
    "severity",
    "status",
    "kind",
    "intent",
    "appearance",
  ].includes(name);
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
      colorType: () => false,
      classes: () => false,
      violations: () => [],
    };
  const checker = program.getTypeChecker();
  const priceFiles = ctx.settings["ds/price-color-files"] ?? [];
  const projectRoot = dirname(
    program.getCompilerOptions().configFilePath ?? ctx.filename,
  );
  const relativeSource = (source) =>
    relative(projectRoot, source.fileName).replaceAll("\\", "/");
  const dsFile = (node) =>
    /^src\/shared\/ds\/.+\.tsx?$/u.test(relativeSource(node.getSourceFile()));
  const generated = (node) =>
    relativeSource(node.getSourceFile()) === "src/shared/ds/tokens.ts";
  const key = JSON.stringify([priceFiles, projectRoot]);
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
  const colorType = (node, seen = new Set(), targetType = "ColorToken") => {
    if (
      !node ||
      seen.has(node) ||
      literals(checker.getTypeAtLocation(node)) === undefined
    )
      return false;
    seen.add(node);
    const resolved = checker.getTypeAtLocation(node);
    const tokenFile = program.getSourceFiles().find((file) => generated(file));
    const tokenModule = tokenFile && checker.getSymbolAtLocation(tokenFile);
    const tokenSymbol =
      tokenModule &&
      checker
        .getExportsOfModule(tokenModule)
        .find((entry) => entry.name === targetType);
    const token = tokenSymbol && checker.getDeclaredTypeOfSymbol(tokenSymbol);
    if (
      token &&
      checker.isTypeAssignableTo(resolved, token) &&
      checker.isTypeAssignableTo(token, resolved)
    )
      return true;
    if (ts.isTypeReferenceNode(node)) {
      if (
        node.typeName.getText() === targetType &&
        aliases(node).some((d) => generated(d))
      )
        return true;
      if (aliases(node).some((d) => colorType(d.type, seen, targetType)))
        return true;
    }
    return (
      ts.forEachChild(
        node,
        (n) => colorType(n, seen, targetType) || undefined,
      ) === true
    );
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
          (classNames.has(base) ||
            overflowKeywords(relativeSource(source)).includes(base)) &&
          !overflowRestrictions(relativeSource(source)).some((r) =>
            new RegExp(r.pattern, "u").test(entry),
          ) &&
          (!/^(text|bg|border|fill|stroke|outline|ring|decoration)-(up|down|on-up|on-down)(-|$)/u.test(
            base,
          ) ||
            priceFiles.includes(
              relative(projectRoot, source.fileName).replaceAll("\\", "/"),
            ))
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
  const ownerOf = (node) => {
    for (let p = node.parent; p !== undefined; p = p.parent)
      if (
        ts.isArrowFunction(p) ||
        ts.isFunctionDeclaration(p) ||
        ts.isFunctionExpression(p)
      )
        return p;
    return undefined;
  };
  const propertyOf = (expression, seen = new Set()) => {
    const node = unwrap(expression);
    if (node === undefined || seen.has(node) || seen.size > 32)
      return undefined;
    seen.add(node);
    if (
      ts.isBinaryExpression(node) &&
      node.operatorToken.kind === ts.SyntaxKind.QuestionQuestionToken
    )
      return propertyOf(node.left, seen);
    if (ts.isIdentifier(node)) {
      const d = (symbol(node)?.declarations ?? [])[0];
      if (d && ts.isBindingElement(d)) {
        const binding = d.parent,
          parent = binding.parent;
        const name = d.propertyName ?? d.name;
        if (!ts.isIdentifier(name)) return undefined;
        if (
          ts.isParameter(parent) &&
          ownerOf(parent)?.parameters[0] === parent
        ) {
          const property = checker.getPropertyOfType(
            checker.getTypeAtLocation(parent),
            name.text,
          );
          return property
            ? { property, parameter: parent, owner: ownerOf(parent) }
            : undefined;
        }
        if (
          ts.isVariableDeclaration(parent) &&
          parent.initializer &&
          ts.isIdentifier(parent.initializer)
        ) {
          const param = (symbol(parent.initializer)?.declarations ?? [])[0];
          if (
            param &&
            ts.isParameter(param) &&
            ownerOf(param)?.parameters[0] === param &&
            (parent.parent.flags & ts.NodeFlags.Const) !== 0
          ) {
            const property = checker.getPropertyOfType(
              checker.getTypeAtLocation(param),
              name.text,
            );
            return property
              ? { property, parameter: param, owner: ownerOf(param) }
              : undefined;
          }
        }
        return undefined;
      }
      if (
        !d ||
        !ts.isVariableDeclaration(d) ||
        (d.parent.flags & ts.NodeFlags.Const) === 0 ||
        ownerOf(d) !== ownerOf(node)
      )
        return undefined;
      return propertyOf(d.initializer, seen);
    }
    if (
      !ts.isPropertyAccessExpression(node) ||
      !ts.isIdentifier(node.expression)
    )
      return undefined;
    const parameter = (symbol(node.expression)?.declarations ?? [])[0];
    if (
      !parameter ||
      !ts.isParameter(parameter) ||
      ownerOf(parameter)?.parameters[0] !== parameter ||
      ownerOf(parameter) !== ownerOf(node)
    )
      return undefined;
    const property = symbol(node.name);
    return property
      ? { property, parameter, owner: ownerOf(parameter) }
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
  const evaluate = (expression) => {
    const node = unwrap(expression);
    if (node === undefined) return undefined;
    const indexed = ts.isElementAccessExpression(node);
    const called = ts.isCallExpression(node) && node.arguments.length === 1;
    if (!indexed && !called) return undefined;
    const argument = indexed ? node.argumentExpression : node.arguments[0];
    const origin = propertyOf(argument);
    const property = origin?.property;
    if (property === undefined) return undefined;
    const keys = indexed
      ? constantObject(node.expression, node.getSourceFile())
      : functionKeys(node.expression, node.getSourceFile());
    const values =
      property === undefined
        ? undefined
        : literals(checker.getTypeOfSymbol(property));
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
    return { declarations, keys, origin, source: node.getSourceFile() };
  };
  const classes = (node) => {
    if (node === undefined) return false;
    if (ts.isConditionalExpression(node))
      return classes(node.whenTrue) && classes(node.whenFalse);
    if (ts.isStringLiteral(node))
      return staticValue(node.text, node.getSourceFile());
    return evaluate(node) !== undefined;
  };
  const sameProperty = (a, b) =>
    a === b ||
    (a.name === b.name &&
      (a.declarations ?? []).some((d) => (b.declarations ?? []).includes(d)));
  const proof = (value, property, owner) => {
    value = unwrap(value);
    if (value === undefined) return false;
    if (ts.isConditionalExpression(value))
      return (
        proof(value.whenTrue, property, owner) &&
        proof(value.whenFalse, property, owner)
      );
    const mapping = evaluate(value);
    return (
      mapping !== undefined &&
      mapping.origin.owner === owner &&
      sameProperty(mapping.origin.property, property)
    );
  };
  const constant = (node, owner, seen) => {
    const d = (symbol(node)?.declarations ?? [])[0];
    if (
      !d ||
      !ts.isVariableDeclaration(d) ||
      !d.initializer ||
      ownerOf(d) !== owner ||
      (d.parent.flags & ts.NodeFlags.Const) === 0 ||
      seen.has(d)
    )
      return undefined;
    seen.add(d);
    return d.initializer;
  };
  const rendered = (node, property, owner, seen = new Set()) => {
    node = unwrap(node);
    if (!node) return false;
    if (ts.isIdentifier(node)) {
      const value = constant(node, owner, seen);
      return value !== undefined && rendered(value, property, owner, seen);
    }
    if (ts.isConditionalExpression(node))
      return (
        rendered(node.whenTrue, property, owner, new Set(seen)) ||
        rendered(node.whenFalse, property, owner, new Set(seen))
      );
    if (
      ts.isBinaryExpression(node) &&
      [
        ts.SyntaxKind.AmpersandAmpersandToken,
        ts.SyntaxKind.BarBarToken,
        ts.SyntaxKind.QuestionQuestionToken,
      ].includes(node.operatorToken.kind)
    )
      return (
        rendered(node.left, property, owner, new Set(seen)) ||
        rendered(node.right, property, owner, new Set(seen))
      );
    if (ts.isJsxExpression(node))
      return rendered(node.expression, property, owner, seen);
    if (ts.isArrayLiteralExpression(node))
      return node.elements.some((child) =>
        rendered(child, property, owner, new Set(seen)),
      );
    if (ts.isJsxFragment(node))
      return node.children.some((child) =>
        rendered(child, property, owner, new Set(seen)),
      );
    if (!ts.isJsxElement(node) && !ts.isJsxSelfClosingElement(node))
      return false;
    const opening = ts.isJsxElement(node) ? node.openingElement : node;
    const own = opening.attributes.properties.some(
      (attribute) =>
        ts.isJsxAttribute(attribute) &&
        attribute.name.getText() === "className" &&
        attribute.initializer &&
        ts.isJsxExpression(attribute.initializer) &&
        proof(attribute.initializer.expression, property, owner),
    );
    return (
      own ||
      (ts.isJsxElement(node) &&
        node.children.some((child) =>
          rendered(child, property, owner, new Set(seen)),
        ))
    );
  };
  const returned = (node, property, owner, seen = new Set()) => {
    node = unwrap(node);
    if (!node) return false;
    if (ts.isIdentifier(node)) {
      const value = constant(node, owner, seen);
      return value !== undefined && returned(value, property, owner, seen);
    }
    if (ts.isConditionalExpression(node))
      return (
        returned(node.whenTrue, property, owner, new Set(seen)) &&
        returned(node.whenFalse, property, owner, new Set(seen))
      );
    return rendered(node, property, owner, seen);
  };
  const functions = [];
  for (const source of program.getSourceFiles()) {
    if (!dsFile(source)) continue;
    const visit = (node) => {
      if (
        (ts.isArrowFunction(node) ||
          ts.isFunctionDeclaration(node) ||
          ts.isFunctionExpression(node)) &&
        node.body
      )
        functions.push(node);
      ts.forEachChild(node, visit);
    };
    visit(source);
  }
  const stringLike = (type, seen = new Set()) => {
    if (seen.has(type)) return false;
    seen.add(type);
    if (type.isUnionOrIntersection())
      return type.types.some((t) => stringLike(t, seen));
    if ((type.flags & ts.TypeFlags.TypeParameter) !== 0) {
      const constraint = checker.getBaseConstraintOfType(type);
      return constraint !== undefined && stringLike(constraint, seen);
    }
    return (type.flags & ts.TypeFlags.StringLike) !== 0;
  };
  const requirements = (owner) => {
    const found = new Set();
    const references = (node) => {
      const binding = ts.isIdentifier(node)
        ? (symbol(node)?.declarations ?? [])[0]
        : undefined;
      const direct =
        ts.isPropertyAccessExpression(node) ||
        (binding &&
          ts.isBindingElement(binding) &&
          ts.isParameter(binding.parent.parent));
      const origin = direct ? propertyOf(node) : undefined;
      if (
        origin &&
        origin.owner === owner &&
        semanticName(origin.property.name) &&
        stringLike(checker.getTypeOfSymbol(origin.property))
      )
        found.add(origin.property);
      ts.forEachChild(node, references);
    };
    const visit = (node) => {
      if (
        node !== owner &&
        (ts.isArrowFunction(node) ||
          ts.isFunctionDeclaration(node) ||
          ts.isFunctionExpression(node))
      )
        return;
      if (
        ts.isJsxAttribute(node) &&
        node.name.getText() === "className" &&
        node.initializer &&
        ts.isJsxExpression(node.initializer) &&
        node.initializer.expression
      )
        references(node.initializer.expression);
      ts.forEachChild(node, visit);
    };
    visit(owner);
    return found;
  };
  const contract = (property, owner) => {
    const values = literals(checker.getTypeOfSymbol(property));
    const declarations = property.declarations ?? [];
    if (
      values === undefined ||
      declarations.length === 0 ||
      !declarations.every((d) => closed(d.type, d.getSourceFile()))
    )
      return false;
    const returns = [];
    const collect = (node) => {
      if (
        node !== owner &&
        (ts.isArrowFunction(node) ||
          ts.isFunctionDeclaration(node) ||
          ts.isFunctionExpression(node))
      )
        return;
      if (ts.isReturnStatement(node)) returns.push(node.expression);
      ts.forEachChild(node, collect);
    };
    if (ts.isBlock(owner.body)) collect(owner);
    else returns.push(owner.body);
    return (
      returns.length > 0 &&
      returns.every((value) => returned(value, property, owner))
    );
  };
  const result = {
    classes,
    colorType,
    violations: (source) =>
      functions.filter(
        (fn) =>
          fn.getSourceFile() === source &&
          [...requirements(fn)].some((property) => !contract(property, fn)),
      ),
  };
  entries.set(key, result);
  return result;
};
