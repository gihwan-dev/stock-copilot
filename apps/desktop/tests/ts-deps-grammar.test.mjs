import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  lockfileProblems,
  yamlLines,
} from "../../../harness/ts-deps/rules.mjs";
import { repoRoot } from "./paths.mjs";

const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
const allowlist = readJson(
  join(repoRoot, "harness", "ts-deps", "allowlist.json"),
);
const lockfile = readFileSync(join(repoRoot, "pnpm-lock.yaml"), "utf8");
const byImporter = {
  ".": readJson(join(repoRoot, "package.json")),
  "apps/desktop": readJson(join(repoRoot, "apps", "desktop", "package.json")),
};

const kindOf = (line) => yamlLines(line)[0].kind;

test("engine ranges that pnpm writes for real packages are in the grammar", () => {
  const lines = [
    "    engines: {node: ^22||^24||>=26}",
    "    engines: {node: ^22.13||^24||>=26}",
    "    engines: {node: ^22 || ^24 || >=26}",
    "    engines: {node: 20 || >=22}",
    "    engines: {node: ^14 <16}",
    "    engines: {node: '>=18', npm: '>=9'}",
    "    engines: {node: ^18.18.0 || ^20.9.0 || >=21.1.0, npm: '>=9'}",
    "    version: npm:typescript@7.0.2",
  ];
  assert.deepEqual(
    lines.filter((line) => kindOf(line) !== "mapping"),
    [],
  );
});

test("forms the grammar rejected before stay outside it", () => {
  const lines = [
    "    'react':",
    '    "react":',
    "    version: &v 19.3.1",
    "    version: *v",
    "\t\treact:",
    "    version: |",
    "    engines: {node: ^14 # comment}",
    "    version: 1.0 2:",
    "    version: >=16",
    "    engines: {'node': ^14}",
    "    engines: {node: ^14,npm: ^9}",
  ];
  assert.deepEqual(
    lines.filter((line) => kindOf(line) !== "unknown"),
    [],
  );
});

test("a flow resolution with a tarball fails even when it is in the grammar", () => {
  const integrity = /resolution: \{integrity: (sha512-[^}]+)\}/;
  const variant = lockfile.replace(
    integrity,
    "resolution: {integrity: $1, tarball: https://example.com/x.tgz}",
  );
  assert.equal(
    kindOf(
      "    resolution: {integrity: sha512-abc, tarball: https://example.com/x.tgz}",
    ),
    "mapping",
  );
  assert.notDeepEqual(lockfileProblems(variant, byImporter, allowlist), []);
});

test("a third document still fails", () => {
  assert.notDeepEqual(
    lockfileProblems(
      `${lockfile}---\nlockfileVersion: '9.0'\n`,
      byImporter,
      allowlist,
    ),
    [],
  );
});
