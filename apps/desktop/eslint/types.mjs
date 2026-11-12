import { relative, sep } from "node:path";
import ts from "typescript";

const F = ts.TypeFlags;

export const projectPath = (context, file) =>
  relative(context.cwd, file).split(sep).join("/");

const sourceFilesOf = (symbol) =>
  (symbol?.declarations ?? []).map((declaration) =>
    declaration.getSourceFile(),
  );

export const declaredIn = (context, symbol, files) =>
  sourceFilesOf(symbol).some((sourceFile) =>
    files.includes(projectPath(context, sourceFile.fileName)),
  );

const isLibraryFile = (sourceFile) =>
  sourceFile.isDeclarationFile ||
  sourceFile.fileName.split(sep).includes("node_modules");

const zodFile = /[\\/]node_modules[\\/]zod[\\/]/u;
const zodBrandFile = /[\\/]zod[\\/]v4[\\/]core[\\/]core\.d\.c?ts$/u;

export const declaredInZod = (symbol) =>
  sourceFilesOf(symbol).some((sourceFile) => zodFile.test(sourceFile.fileName));

const isSymbolKey = (property) =>
  property.escapedName.toString().startsWith("__@");

const isZodBrandProperty = (property) =>
  isSymbolKey(property) &&
  sourceFilesOf(property).some((sourceFile) =>
    zodBrandFile.test(sourceFile.fileName),
  );

const typeOfSymbol = (checker, symbol) => checker.getTypeOfSymbol(symbol);

const partsOf = (type) =>
  type.isUnion() || type.isIntersection() ? type.types : [type];

export const brandNamesOf = (checker, type) => [
  ...new Set(
    partsOf(type).flatMap((part) =>
      part.isUnion() || part.isIntersection()
        ? brandNamesOf(checker, part)
        : checker
            .getPropertiesOfType(part)
            .filter(isZodBrandProperty)
            .flatMap((property) =>
              checker
                .getPropertiesOfType(typeOfSymbol(checker, property))
                .map((name) => name.escapedName.toString()),
            ),
    ),
  ),
];

const isBrandObject = (checker, type) =>
  (type.flags & F.Object) !== 0 &&
  checker.getPropertiesOfType(type).some(isZodBrandProperty);

const PRIMITIVE =
  F.String |
  F.Number |
  F.BigInt |
  F.Boolean |
  F.BooleanLiteral |
  F.TemplateLiteral |
  F.StringMapping;
const OPEN = F.Any | F.Unknown | F.NonPrimitive;
const LITERAL = F.StringLiteral | F.NumberLiteral | F.BigIntLiteral;

const isPrimitiveLike = (type) => (type.flags & (PRIMITIVE | LITERAL)) !== 0;

const isProjectDeclared = (type) => {
  const files = sourceFilesOf(type.getSymbol());
  return files.length > 0 && files.every((file) => !isLibraryFile(file));
};

const isEmptyObject = (checker, type) => {
  const symbol = type.getSymbol();
  const libraryObject =
    symbol?.escapedName === "Object" &&
    sourceFilesOf(symbol).every(isLibraryFile);
  const anonymous =
    (type.objectFlags & ts.ObjectFlags.Anonymous) !== 0 &&
    !sourceFilesOf(symbol).some(isLibraryFile);
  return (
    libraryObject ||
    (anonymous &&
      checker.getPropertiesOfType(type).length === 0 &&
      type.getCallSignatures().length === 0 &&
      type.getConstructSignatures().length === 0 &&
      checker.getIndexInfosOfType(type).length === 0)
  );
};

const MAX_DEPTH = 10;

export const rawTypeChecker = (checker) => {
  const isDefault = (parameter, argument) =>
    parameter !== undefined &&
    checker.getDefaultFromTypeParameter(parameter) === argument;

  const referenceArguments = (type) => {
    if ((type.objectFlags & ts.ObjectFlags.Reference) === 0) return [];
    const parameters = type.target?.typeParameters ?? [];
    return checker
      .getTypeArguments(type)
      .filter((argument, index) => !isDefault(parameters[index], argument));
  };

  const aliasArguments = (type) => {
    const declaration = type.aliasSymbol?.declarations?.[0];
    const parameters = (declaration?.typeParameters ?? []).map((node) =>
      checker.getTypeAtLocation(node),
    );
    return (type.aliasTypeArguments ?? []).filter(
      (argument, index) => !isDefault(parameters[index], argument),
    );
  };

  const signatureRaw = (signature, visit) =>
    signature
      .getParameters()
      .some((parameter) => visit(typeOfSymbol(checker, parameter))) ||
    visit(signature.getReturnType());

  const objectRaw = (type, visit) => {
    if (isEmptyObject(checker, type)) return true;
    if (referenceArguments(type).some(visit)) return true;
    const indexes = checker.getIndexInfosOfType(type);
    if (indexes.some((info) => visit(info.type))) return true;
    if (!isProjectDeclared(type)) return false;
    return (
      indexes.some((info) => visit(info.keyType)) ||
      checker
        .getPropertiesOfType(type)
        .filter((property) => !isSymbolKey(property))
        .some((property) => visit(typeOfSymbol(checker, property))) ||
      [...type.getCallSignatures(), ...type.getConstructSignatures()].some(
        (signature) => signatureRaw(signature, visit),
      )
    );
  };

  const intersectionRaw = (type, visit) => {
    const primitives = type.types.filter(isPrimitiveLike);
    const brands = type.types.filter((part) => isBrandObject(checker, part));
    const rest = type.types.filter(
      (part) => !primitives.includes(part) && !brands.includes(part),
    );
    return primitives.length > 0 && brands.length === 0
      ? true
      : rest.some(visit);
  };

  const isRaw = (root) => {
    const seen = new Set();
    const visit = (type, depth = 0) => {
      if (depth > MAX_DEPTH || seen.has(type)) return false;
      seen.add(type);
      const next = (inner) => visit(inner, depth + 1);
      if ((type.flags & (PRIMITIVE | OPEN)) !== 0) return true;
      if (aliasArguments(type).some(next)) return true;
      if (type.isUnion()) return type.types.some(next);
      if (type.isIntersection()) return intersectionRaw(type, next);
      if ((type.flags & F.Object) !== 0) return objectRaw(type, next);
      return false;
    };
    return visit(root);
  };

  return { isRaw, signatureRaw: (signature) => signatureRaw(signature, isRaw) };
};

export const typeFlags = F;
