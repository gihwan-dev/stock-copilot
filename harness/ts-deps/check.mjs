import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import {
  lockImporters,
  manifestProblems,
  workspacePackages,
} from "./rules.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const root = process.argv[2] ?? join(here, "..", "..");
const read = (path) => readFileSync(join(root, path), "utf8");

const allowlist = JSON.parse(
  readFileSync(join(here, "allowlist.json"), "utf8"),
);
const packages = workspacePackages(read("pnpm-workspace.yaml"));
const manifestPaths = [
  "package.json",
  ...packages.map((folder) => `${folder}/package.json`),
];
const manifests = Object.fromEntries(
  manifestPaths.map((path) => [path, JSON.parse(read(path))]),
);
const importers = lockImporters(read("pnpm-lock.yaml"));
const problems = [
  ...manifestProblems(manifests, allowlist),
  ...(isDeepStrictEqual(importers, [".", ...packages].sort())
    ? []
    : [
        `pnpm-lock.yaml importers must be the workspace packages; got ${importers.join(", ")}`,
      ]),
];

problems.forEach((problem) => process.stdout.write(`FAIL ${problem}\n`));
process.stdout.write(`ts-deps: ${problems.length} problem(s)\n`);
process.exitCode = problems.length === 0 ? 0 : 1;
