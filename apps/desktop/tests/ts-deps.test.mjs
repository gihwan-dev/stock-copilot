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
  lockImporters,
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

test("every allowed package has a reason", () => {
  const missing = Object.entries(allowlist.manifests).flatMap(([path, entry]) =>
    ["dependencies", "devDependencies"].flatMap((section) =>
      Object.entries(entry[section] ?? {})
        .filter(([, reason]) => reason.trim().length === 0)
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

test("lockfile importers are read from every importers section", () => {
  assert.deepEqual(
    lockImporters(readFileSync(join(repoRoot, "pnpm-lock.yaml"), "utf8")),
    [".", "apps/desktop"],
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
