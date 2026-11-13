import { isDeepStrictEqual } from "node:util";

export const TRUSTED_LOCKFILE_FIXTURES = [
  "crates/xtask/src/git_hooks/tests.rs",
];
export const installPolicyProblems = (
  contents,
  allowed = TRUSTED_LOCKFILE_FIXTURES,
) =>
  Object.entries(contents)
    .filter(
      ([path, text]) =>
        !allowed.includes(path) &&
        [
          "trust-" + "lockfile",
          "pnpm_config_trust_" + "lockfile",
          "trustLock" + "file",
        ].some((token) => text.toLowerCase().includes(token.toLowerCase())),
    )
    .map(([path]) => path + ": fixture 밖의 공급망 정책 검증 생략 옵션");

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

const key = String.raw`(?:'@[A-Za-z0-9@._/+()^~-]+'|[A-Za-z0-9._/+-][A-Za-z0-9@._/+()^~-]*)`;
const plain = String.raw`[A-Za-z0-9@._/+()^~<=>-]`;
const token = String.raw`(?:${plain}|:(?=${plain}))*`;
const bare = String.raw`[A-Za-z0-9^~]${token}(?:(?: ?\|\| ?)[A-Za-z0-9^~<>=]${token}| [<>=^~0-9]${token})*`;
const quoted = String.raw`'[^']*'`;
const scalar = `(?:${quoted}|${bare})`;
const flowPair = String.raw`[A-Za-z]+: ${scalar}`;
const flowMap = String.raw`\{(?:${flowPair}(?:, ${flowPair})*)?\}`;
const flowList = String.raw`\[${scalar}(?:, ${scalar})*\]`;
const mappingLine = new RegExp(
  `^((?:  )*)(${key}):(?: (${scalar}|${flowMap}|${flowList}))?$`,
);
const listLine = new RegExp(`^((?:  )+)- (${scalar})$`);

const unquote = (text) =>
  text.startsWith("'") && text.endsWith("'") ? text.slice(1, -1) : text;

export const yamlLines = (text) =>
  text.split("\n").map((line, index) => {
    const mapping = mappingLine.exec(line);
    const item = listLine.exec(line);
    return line === ""
      ? { index, kind: "blank" }
      : line === "---"
        ? { index, kind: "document" }
        : mapping !== null
          ? {
              index,
              kind: "mapping",
              depth: mapping[1].length / 2,
              key: unquote(mapping[2]),
              value: mapping[3] === undefined ? undefined : unquote(mapping[3]),
              raw: mapping[3],
            }
          : item !== null
            ? { index, kind: "item", depth: item[1].length / 2 - 1 }
            : { index, kind: "unknown", text: line };
  });

const grammarProblems = (file, lines) =>
  lines
    .filter((line) => line.kind === "unknown")
    .map(
      (line) =>
        `${file}: line ${line.index + 1} is not in the accepted line grammar: ${JSON.stringify(line.text)}`,
    );

export const workspaceProblems = (workspaceYaml) => {
  const lines = yamlLines(workspaceYaml);
  return [
    ...grammarProblems("pnpm-workspace.yaml", lines),
    ...lines
      .filter((line) => line.kind === "mapping" || line.kind === "document")
      .filter(
        (line) =>
          !(
            line.kind === "mapping" &&
            line.depth === 0 &&
            line.key === "packages" &&
            line.value === undefined
          ),
      )
      .map(
        (line) => `pnpm-workspace.yaml: line ${line.index + 1} is not allowed`,
      ),
  ];
};

export const workspacePackages = (workspaceYaml) =>
  workspaceYaml
    .split("\n")
    .map((line) => new RegExp(`^  - (${scalar})$`).exec(line))
    .filter((match) => match !== null)
    .map((match) => unquote(match[1]));

export const parseLockfile = (lockfile) => {
  const lines = yamlLines(lockfile);
  const documents = lines.filter((line) => line.kind === "document");
  const entries = lines.reduce(
    (state, line) =>
      line.kind === "document"
        ? { doc: state.doc + 1, path: [], list: state.list }
        : line.kind === "mapping"
          ? {
              doc: state.doc,
              path: [...state.path.slice(0, line.depth), line.key],
              list: [
                ...state.list,
                {
                  doc: state.doc,
                  path: [...state.path.slice(0, line.depth), line.key],
                  value: line.value,
                  raw: line.raw,
                },
              ],
            }
          : state,
    { doc: 0, path: [], list: [] },
  ).list;
  return { lines, documents, entries };
};

const integrityOnly = /^\{integrity: sha512-[A-Za-z0-9+/]+={0,2}\}$/;
const registryVersion = String.raw`\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?`;
const lockVersion = new RegExp(
  `^(?:([^()]+)@)?(${registryVersion})(?:\\(.+\\))*$`,
);

const importerDependencies = (entries, doc) =>
  Object.values(
    entries
      .filter(
        (entry) =>
          entry.doc === doc &&
          entry.path.length === 5 &&
          entry.path[0] === "importers",
      )
      .reduce((deps, entry) => {
        const [, importer, section, name, field] = entry.path;
        const id = `${importer} ${section} ${name}`;
        return {
          ...deps,
          [id]: {
            importer,
            section,
            name,
            ...(deps[id] ?? {}),
            [field]: entry.value,
          },
        };
      }, {}),
  );

const manifestPathOf = (importer) =>
  importer === "." ? "package.json" : `${importer}/package.json`;

const packageResolutions = (entries) =>
  entries
    .filter((entry) => entry.path[0] === "packages" && entry.path.length === 2)
    .map((entry) => ({
      key: entry.path[1],
      doc: entry.doc,
      resolutions: entries
        .filter(
          (other) =>
            other.doc === entry.doc &&
            other.path.length === 3 &&
            other.path[0] === "packages" &&
            other.path[1] === entry.path[1] &&
            other.path[2] === "resolution",
        )
        .map((other) => other.raw),
    }));

const versionProblems = (dep, target, packages) => {
  const match = lockVersion.exec(dep.version ?? "");
  const where = `pnpm-lock.yaml: ${dep.importer} ${dep.section} ${dep.name}`;
  if (match === null)
    return [
      `${where} version must be an exact registry version; got ${dep.version}`,
    ];
  const [, named, version] = match;
  const expectedName = named ?? dep.name;
  return [
    ...(target === undefined || expectedName === target
      ? []
      : [`${where} version points at ${expectedName}, expected ${target}`]),
    ...(target !== undefined && target !== dep.name && named === undefined
      ? [`${where} version must name the alias target ${target}`]
      : []),
    ...(packages.some(
      (entry) =>
        entry.key === `${expectedName}@${version}` &&
        entry.resolutions.length === 1 &&
        integrityOnly.test(entry.resolutions[0]),
    )
      ? []
      : [
          `${where} has no packages entry ${expectedName}@${version} with a sha512 integrity`,
        ]),
  ];
};

export const lockfileProblems = (lockfile, manifestsByImporter, allowlist) => {
  const { lines, documents, entries } = parseLockfile(lockfile);
  const packages = packageResolutions(entries);
  const lastDoc = documents.length;
  const topKeys = entries
    .filter((entry) => entry.path.length === 1)
    .map((entry) => entry.path[0]);
  const pnpmVersion = /^pnpm@(\d+\.\d+\.\d+)\+/.exec(
    allowlist.packageManager,
  )?.[1];
  const firstDocDeps = importerDependencies(entries, 1);
  const deps = importerDependencies(entries, lastDoc).filter((dep) =>
    dependencySections.includes(dep.section),
  );
  const lockDeps = deps
    .map((dep) => `${dep.importer} ${dep.section} ${dep.name} ${dep.specifier}`)
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
    ...grammarProblems("pnpm-lock.yaml", lines),
    ...packages
      .filter(
        (entry) =>
          !entry.resolutions.every((resolution) =>
            integrityOnly.test(resolution),
          ),
      )
      .map(
        (entry) =>
          `pnpm-lock.yaml: packages ${entry.key} resolution must be a single sha512 integrity`,
      ),
    ...(lines[0]?.kind === "document" && documents.length === 2
      ? []
      : [
          `pnpm-lock.yaml must be exactly two documents (package manager, then dependencies); got ${documents.length} separator(s)`,
        ]),
    ...topKeys
      .filter((name) => !allowlist.lockfileTopLevelKeys.includes(name))
      .map((name) => `pnpm-lock.yaml: top-level key "${name}" is not allowed`),
    ...entries
      .filter(
        (entry) =>
          entry.doc === 1 &&
          entry.path[0] === "importers" &&
          entry.path.length >= 2 &&
          !(
            entry.path[1] === "." &&
            (entry.path.length === 2 ||
              (entry.path[2] === "configDependencies" &&
                entry.path.length === 3 &&
                entry.raw === "{}") ||
              (entry.path[2] === "packageManagerDependencies" &&
                (entry.path.length === 3 || entry.path[3] === "pnpm")))
          ),
      )
      .map(
        (entry) =>
          `pnpm-lock.yaml: first document may only pin pnpm itself; got ${entry.path.join(" ")}`,
      ),
    ...firstDocDeps
      .filter((dep) => dep.section === "packageManagerDependencies")
      .flatMap((dep) => [
        ...(dep.specifier === pnpmVersion && dep.version === pnpmVersion
          ? []
          : [
              `pnpm-lock.yaml: package manager pin must be ${pnpmVersion}; got ${dep.specifier} / ${dep.version}`,
            ]),
        ...versionProblems(dep, "pnpm", packages),
      ]),
    ...packages
      .filter(
        (entry) =>
          entry.resolutions.length !== 1 ||
          !integrityOnly.test(entry.resolutions[0]),
      )
      .map(
        (entry) =>
          `pnpm-lock.yaml: resolution of ${entry.key} must be a sha512 integrity only; got ${entry.resolutions.join(" ") || "none"}`,
      ),
    ...deps.flatMap((dep) =>
      versionProblems(
        dep,
        allowlist.manifests[manifestPathOf(dep.importer)]?.[dep.section]?.[
          dep.name
        ]?.target,
        packages,
      ),
    ),
    ...(isDeepStrictEqual(lockDeps, manifestDeps)
      ? []
      : [
          `pnpm-lock.yaml importers must list exactly the manifest specifiers; lock has [${lockDeps.join("; ")}], manifests have [${manifestDeps.join("; ")}]`,
        ]),
  ];
};
