import { isDeepStrictEqual } from "node:util";

export const dependencySections = [
  "dependencies",
  "devDependencies",
  "optionalDependencies",
  "peerDependencies",
  "bundleDependencies",
  "bundledDependencies",
];

const keyProblems = (path, manifest, allowed) =>
  isDeepStrictEqual(Object.keys(manifest).sort(), [...allowed.keys].sort())
    ? []
    : [
        `${path}: top-level keys must be exactly ${[...allowed.keys].sort().join(", ")}; got ${Object.keys(manifest).sort().join(", ")}`,
      ];

const sectionProblems = (path, manifest, allowed) =>
  dependencySections.flatMap((section) =>
    Object.keys(manifest[section] ?? {})
      .filter((name) => !Object.hasOwn(allowed[section] ?? {}, name))
      .map(
        (name) =>
          `${path}: ${section} "${name}" is not in harness/ts-deps/allowlist.json`,
      ),
  );

export const manifestProblems = (manifests, allowlist) => {
  const listed = Object.keys(allowlist.manifests).sort();
  const found = Object.keys(manifests).sort();
  return [
    ...(isDeepStrictEqual(found, listed)
      ? []
      : [
          `workspace manifests must be exactly ${listed.join(", ")}; got ${found.join(", ")}`,
        ]),
    ...found
      .filter((path) => Object.hasOwn(allowlist.manifests, path))
      .flatMap((path) => [
        ...keyProblems(path, manifests[path], allowlist.manifests[path]),
        ...sectionProblems(path, manifests[path], allowlist.manifests[path]),
      ]),
    ...(manifests["package.json"]?.packageManager === allowlist.packageManager
      ? []
      : [
          `package.json: packageManager must be ${allowlist.packageManager}; got ${manifests["package.json"]?.packageManager}`,
        ]),
  ];
};

export const workspacePackages = (workspaceYaml) =>
  workspaceYaml
    .split("\n")
    .map((line) => /^\s+-\s+(\S+)\s*$/.exec(line))
    .filter((match) => match !== null)
    .map((match) => match[1].replace(/^["']|["']$/g, ""));

const importerSection = (lines, start) => {
  const rest = lines.slice(start + 1);
  const end = rest.findIndex((line) => /^\S/.test(line));
  return end === -1 ? rest : rest.slice(0, end);
};

export const lockImporters = (lockfile) => {
  const lines = lockfile.split("\n");
  return [
    ...new Set(
      lines
        .flatMap((line, index) =>
          line === "importers:" ? importerSection(lines, index) : [],
        )
        .map((line) => /^ {2}(\S[^:]*):\s*$/.exec(line))
        .filter((match) => match !== null)
        .map((match) => match[1].replace(/^["']|["']$/g, "")),
    ),
  ].sort();
};
