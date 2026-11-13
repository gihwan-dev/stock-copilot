export const PACKAGE_DEPENDENCY_KEYS = [
  "dependencies",
  "devDependencies",
  "optionalDependencies",
  "peerDependencies",
];

export const PLAIN_PACKAGE_SPEC =
  /^(?:npm:(?:@[a-z0-9][a-z0-9._-]*\/)?[a-z0-9][a-z0-9._-]*@)?[~^]?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/u;

const plainDependencies = (text) => {
  const parsed = JSON.parse(text);
  return PACKAGE_DEPENDENCY_KEYS.every((key) => {
    const entries = parsed[key];
    return (
      entries === undefined ||
      (typeof entries === "object" &&
        entries !== null &&
        !Array.isArray(entries) &&
        Object.values(entries).every(
          (spec) => typeof spec === "string" && PLAIN_PACKAGE_SPEC.test(spec),
        ))
    );
  });
};

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
  const name = (path.split("/").at(-1) ?? "").toLowerCase();
  return name === "cargo.toml"
    ? "cargo"
    : name === "package.json"
      ? "package"
      : undefined;
};

const escapedPiece = (piece) => piece.replace(/[.+?^${}()|[\]\\]/gu, "\\$&");

export const configNameMatches = (names, path) => {
  const name = (path.split("/").at(-1) ?? "").toLowerCase();
  return names.some((pattern) =>
    new RegExp(
      `^${pattern.toLowerCase().split("*").map(escapedPiece).join(".*")}$`,
      "u",
    ).test(name),
  );
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
  return (
    left === undefined ||
    right === undefined ||
    left !== right ||
    !plainDependencies(after)
  );
};
