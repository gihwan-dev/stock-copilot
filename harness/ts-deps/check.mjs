import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  fileProblems,
  lockfileProblems,
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
const importers = [".", ...packages];
const manifestPath = (importer) =>
  importer === "." ? "package.json" : `${importer}/package.json`;
const manifests = Object.fromEntries(
  importers.map((importer) => [
    manifestPath(importer),
    JSON.parse(read(manifestPath(importer))),
  ]),
);
const watched = [...Object.keys(allowlist.files), ...allowlist.forbiddenFiles];
const contents = Object.fromEntries(
  watched
    .filter((path) => existsSync(join(root, path)))
    .map((path) => [path, read(path)]),
);
const problems = [
  ...manifestProblems(manifests, allowlist),
  ...fileProblems(contents, allowlist),
  ...lockfileProblems(
    read("pnpm-lock.yaml"),
    Object.fromEntries(
      importers.map((importer) => [
        importer,
        manifests[manifestPath(importer)],
      ]),
    ),
    allowlist,
  ),
];

problems.forEach((problem) => process.stdout.write(`FAIL ${problem}\n`));
process.stdout.write(`ts-deps: ${problems.length} problem(s)\n`);
process.exitCode = problems.length === 0 ? 0 : 1;
