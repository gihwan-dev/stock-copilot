export const PACKAGE_DEPENDENCY_KEYS = [
  "dependencies",
  "devDependencies",
  "optionalDependencies",
  "peerDependencies",
];

const packageOutsideDependencies = (text) => {
  try {
    const parsed = JSON.parse(text);
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      Array.isArray(parsed)
    ) {
      return undefined;
    }
    return JSON.stringify(
      Object.fromEntries(
        Object.entries(parsed).filter(
          ([key]) => !PACKAGE_DEPENDENCY_KEYS.includes(key),
        ),
      ),
    );
  } catch {
    return undefined;
  }
};

export const manifestKind = (path) => {
  const name = path.split("/").at(-1);
  return name === "Cargo.toml"
    ? "cargo"
    : name === "package.json"
      ? "package"
      : undefined;
};

export const packageChangesOutsideDependencies = (before, after) => {
  if (before === after) {
    return false;
  }
  if (before === undefined || after === undefined) {
    return true;
  }
  const left = packageOutsideDependencies(before);
  const right = packageOutsideDependencies(after);
  return left === undefined || right === undefined || left !== right;
};
