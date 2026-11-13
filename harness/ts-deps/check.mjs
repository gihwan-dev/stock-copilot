import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  fileProblems,
  lockfileProblems,
  manifestProblems,
  workspaceProblems,
  workspacePackages,
  installPolicyProblems,
} from "./rules.mjs";
import { git } from "../../.claude/hooks/lib/project.mjs";

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
const tracked = git(root, ["ls-files", "-z"]);
const policyProblems =
  tracked === undefined
    ? ["공급망 정책 검사 대상의 Git 목록을 읽지 못했다"]
    : installPolicyProblems(
        Object.fromEntries(
          tracked
            .split("\0")
            .filter((path) => path && !path.startsWith("docs/"))
            .map((path) => [path, read(path)]),
        ),
      );
const problems = [
  ...policyProblems,
  ...manifestProblems(manifests, allowlist),
  ...fileProblems(contents, allowlist),
  ...workspaceProblems(read("pnpm-workspace.yaml")),
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
