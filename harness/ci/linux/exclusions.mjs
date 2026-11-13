import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const EXCLUSIONS_FILE = "harness/ci/linux/exclusions.json";

const CRATE_FIELDS = ["name", "reason"];

const TEST_FIELDS = ["package", "module", "reason"];

export const readExclusions = (root) =>
  JSON.parse(readFileSync(join(root, EXCLUSIONS_FILE), "utf8"));

export const readMetadata = (root) => {
  const run = spawnSync(
    "cargo",
    ["metadata", "--no-deps", "--locked", "--format-version", "1"],
    { cwd: root, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  );
  return run.status === 0
    ? JSON.parse(run.stdout)
    : { problem: run.stderr.trim() };
};

export const cargoArgs = (exclusions) =>
  exclusions.crates.flatMap((entry) => ["--exclude", entry.name]);

export const nextestFilter = (exclusions) => {
  const packages = [
    ...new Set(exclusions.tests.map((entry) => entry.package)),
  ].sort();
  if (packages.length === 0) {
    return "all()";
  }
  const groups = packages.map((name) => {
    const modules = exclusions.tests
      .filter((entry) => entry.package === name)
      .map((entry) => `test(/^${entry.module}::/)`)
      .join(" | ");
    return `(package(${name}) & (${modules}))`;
  });
  return `not (${groups.join(" | ")})`;
};

const exactFields = (entry, fields) =>
  typeof entry === "object" &&
  entry !== null &&
  Object.keys(entry).length === fields.length &&
  fields.every(
    (field) => typeof entry[field] === "string" && entry[field].trim() !== "",
  );

const shapeProblems = (exclusions) => {
  if (
    typeof exclusions !== "object" ||
    exclusions === null ||
    !Array.isArray(exclusions.crates) ||
    !Array.isArray(exclusions.tests) ||
    Object.keys(exclusions).length !== 2
  ) {
    return [`${EXCLUSIONS_FILE}: 최상위는 crates와 tests 배열이다`];
  }
  return [
    ...exclusions.crates
      .filter((entry) => !exactFields(entry, CRATE_FIELDS))
      .map((_, index) => `crates[${index}]: name과 reason만 가진다`),
    ...exclusions.tests
      .filter((entry) => !exactFields(entry, TEST_FIELDS))
      .map((_, index) => `tests[${index}]: package, module, reason만 가진다`),
  ];
};

const duplicates = (keys) =>
  keys.filter((key, index) => keys.indexOf(key) !== index);

const declaresModule = (root, member, module) =>
  member.targets
    .map((target) => target.src_path)
    .filter((path) => existsSync(path))
    .some((path) =>
      new RegExp(`^mod ${module};$`, "mu").test(readFileSync(path, "utf8")),
    );

export const exclusionProblems = (root, exclusions, metadata) => {
  const shapes = shapeProblems(exclusions);
  if (shapes.length > 0) {
    return shapes;
  }
  if (metadata.problem !== undefined) {
    return [`cargo metadata가 실패했다: ${metadata.problem}`];
  }
  const members = new Map(
    metadata.packages.map((member) => [member.name, member]),
  );
  const excluded = new Set(exclusions.crates.map((entry) => entry.name));
  const unknown = exclusions.crates
    .filter((entry) => !members.has(entry.name))
    .map((entry) => `${entry.name}: 작업 공간 crate가 아니다`);
  const dependents = metadata.packages
    .filter((member) => !excluded.has(member.name))
    .flatMap((member) =>
      member.dependencies
        .filter((dependency) => excluded.has(dependency.name))
        .map(
          (dependency) =>
            `${member.name}: 제외된 ${dependency.name}에 의존하므로 함께 제외해야 한다`,
        ),
    );
  const tests = exclusions.tests.flatMap((entry) => {
    const member = members.get(entry.package);
    if (member === undefined) {
      return [`${entry.package}: 작업 공간 crate가 아니다`];
    }
    if (excluded.has(entry.package)) {
      return [
        `${entry.package}: crate 전체가 제외되어 테스트 제외가 필요 없다`,
      ];
    }
    return declaresModule(root, member, entry.module)
      ? []
      : [`${entry.package}::${entry.module}: 그런 모듈 선언이 없다`];
  });
  return [
    ...duplicates(exclusions.crates.map((entry) => entry.name)).map(
      (name) => `${name}: 두 번 있다`,
    ),
    ...duplicates(
      exclusions.tests.map((entry) => `${entry.package}::${entry.module}`),
    ).map((name) => `${name}: 두 번 있다`),
    ...unknown,
    ...dependents,
    ...tests,
  ];
};

const repoRoot = resolve(
  fileURLToPath(import.meta.url),
  "..",
  "..",
  "..",
  "..",
);

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const exclusions = readExclusions(repoRoot);
  const [command] = process.argv.slice(2);
  if (command === "cargo") {
    process.stdout.write(`${cargoArgs(exclusions).join(" ")}\n`);
  } else if (command === "filter") {
    process.stdout.write(`${nextestFilter(exclusions)}\n`);
  } else if (command === "verify") {
    const problems = exclusionProblems(
      repoRoot,
      exclusions,
      readMetadata(repoRoot),
    );
    problems.forEach((problem) => process.stdout.write(`FAIL ${problem}\n`));
    process.stdout.write(`linux-exclusions: ${problems.length} failure(s)\n`);
    process.exitCode = problems.length === 0 ? 0 : 1;
  } else {
    process.stderr.write(
      "사용법: node harness/ci/linux/exclusions.mjs cargo | filter | verify\n",
    );
    process.exitCode = 2;
  }
}
