import { isDeepStrictEqual } from "node:util";

export const dependencySections = [
  "dependencies",
  "devDependencies",
  "optionalDependencies",
  "peerDependencies",
  "bundleDependencies",
  "bundledDependencies",
];

const exactVersion = /^\d+\.\d+\.\d+$/;

export const specProblem = (name, spec, target) => {
  const expected = target === name ? "<x.y.z>" : `npm:${target}@<x.y.z>`;
  const version =
    target === name
      ? spec
      : typeof spec === "string" && spec.startsWith(`npm:${target}@`)
        ? spec.slice(`npm:${target}@`.length)
        : undefined;
  return typeof version === "string" && exactVersion.test(version)
    ? []
    : [`"${name}" must be ${expected}; got ${JSON.stringify(spec)}`];
};

const keyProblems = (path, manifest, allowed) =>
  isDeepStrictEqual(Object.keys(manifest).sort(), [...allowed.keys].sort())
    ? []
    : [
        `${path}: top-level keys must be exactly ${[...allowed.keys].sort().join(", ")}; got ${Object.keys(manifest).sort().join(", ")}`,
      ];

const sectionProblems = (path, manifest, allowed) =>
  dependencySections.flatMap((section) =>
    Object.entries(manifest[section] ?? {}).flatMap(([name, spec]) =>
      Object.hasOwn(allowed[section] ?? {}, name)
        ? specProblem(name, spec, allowed[section][name].target).map(
            (problem) => `${path}: ${section} ${problem}`,
          )
        : [
            `${path}: ${section} "${name}" is not in harness/ts-deps/allowlist.json`,
          ],
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
    ...(/^pnpm@\d+\.\d+\.\d+\+sha512\.[0-9a-f]{128}$/.test(
      allowlist.packageManager,
    )
      ? []
      : [
          `allowlist packageManager must carry a sha512 hash; got ${allowlist.packageManager}`,
        ]),
  ];
};

export const fileProblems = (contents, allowlist) => [
  ...Object.entries(allowlist.files)
    .filter(([path, expected]) => contents[path] !== expected)
    .map(
      ([path]) =>
        `${path} must equal the content in harness/ts-deps/allowlist.json`,
    ),
  ...allowlist.forbiddenFiles
    .filter((path) => contents[path] !== undefined)
    .map((path) => `${path} must not exist`),
];

export const workspacePackages = (workspaceYaml) =>
  workspaceYaml
    .split("\n")
    .map((line) => /^\s+-\s+(\S+)\s*$/.exec(line))
    .filter((match) => match !== null)
    .map((match) => match[1].replace(/^["']|["']$/g, ""));

const unquote = (text) => text.replace(/^["']|["']$/g, "");

export const parseLockfile = (lockfile) =>
  lockfile.split("\n").reduce(
    (state, line) => {
      const top = /^([A-Za-z][\w-]*):/.exec(line);
      const importer = /^ {2}(\S[^:]*):\s*$/.exec(line);
      const section = /^ {4}(\w+):(\s*\{\})?\s*$/.exec(line);
      const name = /^ {6}(\S[^:]*):\s*$/.exec(line);
      const specifier = /^ {8}specifier: (.+)$/.exec(line);
      const resolution = /^ {4}resolution: (.+)$/.exec(line);
      return top !== null
        ? {
            ...state,
            block: top[1],
            topKeys: [...new Set([...state.topKeys, top[1]])],
          }
        : state.block === "importers" && importer !== null
          ? { ...state, importer: unquote(importer[1]) }
          : state.block === "importers" && section !== null
            ? { ...state, section: section[1] }
            : state.block === "importers" && name !== null
              ? { ...state, name: unquote(name[1]) }
              : state.block === "importers" && specifier !== null
                ? {
                    ...state,
                    specifiers: [
                      ...state.specifiers,
                      {
                        importer: state.importer,
                        section: state.section,
                        name: state.name,
                        specifier: unquote(specifier[1]),
                      },
                    ],
                  }
                : state.block === "packages" && resolution !== null
                  ? {
                      ...state,
                      resolutions: [...state.resolutions, resolution[1]],
                    }
                  : state;
    },
    {
      block: "",
      importer: "",
      section: "",
      name: "",
      topKeys: [],
      specifiers: [],
      resolutions: [],
    },
  );

const integrityOnly = /^\{integrity: sha512-[A-Za-z0-9+/]+={0,2}\}$/;

export const lockfileProblems = (lockfile, manifestsByImporter, allowlist) => {
  const parsed = parseLockfile(lockfile);
  const lockDeps = parsed.specifiers
    .filter((entry) => dependencySections.includes(entry.section))
    .map(
      (entry) =>
        `${entry.importer} ${entry.section} ${entry.name} ${entry.specifier}`,
    )
    .sort();
  const manifestDeps = Object.entries(manifestsByImporter)
    .flatMap(([importer, manifest]) =>
      dependencySections.flatMap((section) =>
        Object.entries(manifest[section] ?? {}).map(
          ([name, spec]) => `${importer} ${section} ${name} ${spec}`,
        ),
      ),
    )
    .sort();
  return [
    ...parsed.topKeys
      .filter((key) => !allowlist.lockfileTopLevelKeys.includes(key))
      .map((key) => `pnpm-lock.yaml: top-level key "${key}" is not allowed`),
    ...parsed.resolutions
      .filter((resolution) => !integrityOnly.test(resolution))
      .map(
        (resolution) =>
          `pnpm-lock.yaml: resolution must be a sha512 integrity only; got ${resolution}`,
      ),
    ...(isDeepStrictEqual(lockDeps, manifestDeps)
      ? []
      : [
          `pnpm-lock.yaml importers must list exactly the manifest specifiers; lock has [${lockDeps.join("; ")}], manifests have [${manifestDeps.join("; ")}]`,
        ]),
  ];
};
