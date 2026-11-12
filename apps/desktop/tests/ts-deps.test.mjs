import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  fileProblems,
  lockfileProblems,
  manifestProblems,
} from "../../../harness/ts-deps/rules.mjs";
import { repoRoot } from "./paths.mjs";

const checkScript = join(repoRoot, "harness", "ts-deps", "check.mjs");
const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
const allowlist = readJson(
  join(repoRoot, "harness", "ts-deps", "allowlist.json"),
);
const manifests = {
  "package.json": readJson(join(repoRoot, "package.json")),
  "apps/desktop/package.json": readJson(
    join(repoRoot, "apps", "desktop", "package.json"),
  ),
};

const withDependency = (path, section, name) => ({
  ...manifests,
  [path]: {
    ...manifests[path],
    [section]: { ...(manifests[path][section] ?? {}), [name]: "1.0.0" },
  },
});

const runCheck = (root) =>
  spawnSync(process.execPath, [checkScript, root], { encoding: "utf8" });

const copyWorkspace = () => {
  const root = mkdtempSync(join(tmpdir(), "ts-deps-"));
  mkdirSync(join(root, "apps", "desktop"), { recursive: true });
  [
    "package.json",
    "pnpm-workspace.yaml",
    "pnpm-lock.yaml",
    "apps/desktop/package.json",
  ].forEach((path) => copyFileSync(join(repoRoot, path), join(root, path)));
  return root;
};

test("the repository manifests pass the TypeScript dependency allowlist", () => {
  const result = runCheck(repoRoot);
  assert.equal(result.status, 0, result.stdout);
  assert.deepEqual(manifestProblems(manifests, allowlist), []);
});

test("every allowed package has a target and a reason", () => {
  const missing = Object.entries(allowlist.manifests).flatMap(([path, entry]) =>
    ["dependencies", "devDependencies"].flatMap((section) =>
      Object.entries(entry[section] ?? {})
        .filter(
          ([, allowed]) =>
            allowed.reason.trim().length === 0 ||
            allowed.target.trim().length === 0,
        )
        .map(([name]) => `${path} ${section} ${name}`),
    ),
  );
  assert.deepEqual(missing, []);
});

test("an unlisted direct dependency fails in every manifest and section", () => {
  const sections = [
    "dependencies",
    "devDependencies",
    "optionalDependencies",
    "peerDependencies",
  ];
  const passing = Object.keys(manifests).flatMap((path) =>
    sections
      .filter(
        (section) =>
          manifestProblems(withDependency(path, section, "left-pad"), allowlist)
            .length === 0,
      )
      .map((section) => `${path} ${section}`),
  );
  assert.deepEqual(passing, []);
});

test("a listed package in the wrong section fails", () => {
  assert.notDeepEqual(
    manifestProblems(
      withDependency("apps/desktop/package.json", "dependencies", "eslint"),
      allowlist,
    ),
    [],
  );
});

test("overrides, scripts and other manifest keys outside the list fail", () => {
  const extra = ["pnpm", "overrides", "resolutions", "scripts"].filter(
    (key) =>
      manifestProblems(
        {
          ...manifests,
          "apps/desktop/package.json": {
            ...manifests["apps/desktop/package.json"],
            [key]: {},
          },
        },
        allowlist,
      ).length === 0,
  );
  assert.deepEqual(extra, []);
});

test("an unlisted workspace manifest fails", () => {
  assert.notDeepEqual(
    manifestProblems(
      { ...manifests, "apps/other/package.json": { name: "other" } },
      allowlist,
    ),
    [],
  );
});

test("packageManager must equal the protected allowlist value", () => {
  assert.notDeepEqual(
    manifestProblems(
      {
        ...manifests,
        "package.json": {
          ...manifests["package.json"],
          packageManager: "pnpm@12.17.0",
        },
      },
      allowlist,
    ),
    [],
  );
});

const lockfile = readFileSync(join(repoRoot, "pnpm-lock.yaml"), "utf8");
const byImporter = {
  ".": manifests["package.json"],
  "apps/desktop": manifests["apps/desktop/package.json"],
};
const fileContents = {
  "pnpm-workspace.yaml": readFileSync(
    join(repoRoot, "pnpm-workspace.yaml"),
    "utf8",
  ),
  ".npmrc": readFileSync(join(repoRoot, ".npmrc"), "utf8"),
};

const withSpec = (path, section, name, spec) => ({
  ...manifests,
  [path]: {
    ...manifests[path],
    [section]: { ...(manifests[path][section] ?? {}), [name]: spec },
  },
});

test("a listed name pointing at another package, a file, git, a URL or the workspace fails", () => {
  const specs = [
    "npm:left-pad@1.3.0",
    "file:../left-pad",
    "link:../left-pad",
    "git+https://github.com/x/react.git",
    "github:x/react",
    "https://example.com/react.tgz",
    "workspace:*",
    "^19.3.1",
    "latest",
  ];
  const targets = [
    ["package.json", "devDependencies", "prettier"],
    ["apps/desktop/package.json", "dependencies", "react"],
    ["apps/desktop/package.json", "devDependencies", "eslint"],
  ];
  const passing = targets.flatMap(([path, section, name]) =>
    specs
      .filter(
        (spec) =>
          manifestProblems(withSpec(path, section, name, spec), allowlist)
            .length === 0,
      )
      .map((spec) => `${path} ${section} ${name} ${spec}`),
  );
  assert.deepEqual(passing, []);
});

test("an npm alias must point at the listed target", () => {
  assert.notDeepEqual(
    manifestProblems(
      withSpec(
        "apps/desktop/package.json",
        "devDependencies",
        "typescript",
        "npm:typescript@7.0.2",
      ),
      allowlist,
    ),
    [],
  );
  assert.deepEqual(
    manifestProblems(
      withSpec(
        "apps/desktop/package.json",
        "devDependencies",
        "typescript",
        "npm:@typescript/typescript6@6.0.3",
      ),
      allowlist,
    ),
    [],
  );
});

test("packageManager without the sha512 hash fails", () => {
  assert.notDeepEqual(
    manifestProblems(
      {
        ...manifests,
        "package.json": {
          ...manifests["package.json"],
          packageManager: "pnpm@12.18.0",
        },
      },
      allowlist,
    ),
    [],
  );
  assert.notDeepEqual(
    manifestProblems(manifests, {
      ...allowlist,
      packageManager: "pnpm@12.18.0",
    }),
    [],
  );
});

test("pnpm-workspace.yaml keys that change resolution fail", () => {
  const passing = [
    "overrides:\n  react: 18.0.0\n",
    "packageExtensions:\n  react:\n    dependencies:\n      left-pad: 1.3.0\n",
    "patchedDependencies:\n  react: patches/react.patch\n",
    "catalog:\n  react: 18.0.0\n",
    "minimumReleaseAge: 0\n",
  ].filter(
    (extra) =>
      fileProblems(
        {
          ...fileContents,
          "pnpm-workspace.yaml": `${fileContents["pnpm-workspace.yaml"]}${extra}`,
        },
        allowlist,
      ).length === 0,
  );
  assert.deepEqual(passing, []);
});

test("a pnpmfile or a changed .npmrc fails", () => {
  assert.deepEqual(fileProblems(fileContents, allowlist), []);
  assert.notDeepEqual(
    fileProblems({ ...fileContents, ".pnpmfile.cjs": "" }, allowlist),
    [],
  );
  assert.notDeepEqual(
    fileProblems(
      {
        ...fileContents,
        ".npmrc": `${fileContents[".npmrc"]}registry=https://example.com/\n`,
      },
      allowlist,
    ),
    [],
  );
  assert.notDeepEqual(
    fileProblems(
      {
        ...fileContents,
        "apps/desktop/.npmrc": "registry=https://example.com/\n",
      },
      allowlist,
    ),
    [],
  );
});

test("the repository lockfile passes", () => {
  assert.deepEqual(lockfileProblems(lockfile, byImporter, allowlist), []);
});

test("a lockfile resolution other than a sha512 integrity fails", () => {
  const integrity = /resolution: \{integrity: sha512-[^}]+\}/;
  const passing = [
    "resolution: {tarball: https://example.com/react.tgz}",
    "resolution: {type: git, repo: https://github.com/x/react.git, commit: abc}",
    "resolution: {directory: ../react, type: directory}",
    "resolution: {integrity: sha1-abc=}",
  ].filter(
    (replacement) =>
      lockfileProblems(
        lockfile.replace(integrity, replacement),
        byImporter,
        allowlist,
      ).length === 0,
  );
  assert.deepEqual(passing, []);
});

test("a lockfile importer specifier that differs from the manifest fails", () => {
  assert.notDeepEqual(
    lockfileProblems(
      lockfile.replace(
        "      react:\n        specifier: 19.3.1",
        "      react:\n        specifier: npm:left-pad@1.3.0",
      ),
      byImporter,
      allowlist,
    ),
    [],
  );
});

test("an unexpected lockfile top-level key fails", () => {
  assert.notDeepEqual(
    lockfileProblems(
      `${lockfile}\noverrides:\n  react: 18.0.0\n`,
      byImporter,
      allowlist,
    ),
    [],
  );
});

test("adding an unlisted package to package.json makes the check script fail", () => {
  const root = copyWorkspace();
  const path = join(root, "apps", "desktop", "package.json");
  const manifest = readJson(path);
  writeFileSync(
    path,
    JSON.stringify({
      ...manifest,
      dependencies: { ...manifest.dependencies, "left-pad": "1.3.0" },
    }),
  );
  const result = runCheck(root);
  rmSync(root, { recursive: true, force: true });
  assert.equal(result.status, 1);
  assert.match(result.stdout, /dependencies "left-pad" is not in/);
});

test("just check runs the TypeScript dependency allowlist check", () => {
  const justfile = readFileSync(join(repoRoot, "justfile"), "utf8");
  assert.match(justfile, /^ {4}node harness\/ts-deps\/check\.mjs$/m);
});
