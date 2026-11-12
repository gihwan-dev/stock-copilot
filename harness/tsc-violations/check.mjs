import { isDeepStrictEqual } from "node:util";
import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { appConfigProblems } from "./app-config.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const desktop = join(here, "..", "..", "apps", "desktop");
const readJson = (path) => JSON.parse(readFileSync(path, "utf8"));
const expected = readJson(join(here, "expected.json"));
const compilers = ["tsc", "tsc6"];
const diagnostic = /^(\S.*?)\(\d+,\d+\): error (TS\d+):/;
const failures = [];
const report = (status, text) => {
  process.stdout.write(`${status} ${text}\n`);
  if (status === "FAIL") failures.push(text);
};

const appProblems = appConfigProblems(
  readJson(join(desktop, "tsconfig.json")),
  expected,
);
for (const problem of appProblems)
  report("FAIL", `apps/desktop/tsconfig.json ${problem}`);
report(
  appProblems.length === 0 ? "PASS" : "FAIL",
  `apps/desktop/tsconfig.json matches expected.json exactly: ${appProblems.length} problem(s)`,
);

const ownConfig = readJson(join(here, "tsconfig.json"));
report(
  isDeepStrictEqual(ownConfig, {
    extends: "../../apps/desktop/tsconfig.json",
    compilerOptions: { rootDir: "." },
    include: ["cases", "support"],
  })
    ? "PASS"
    : "FAIL",
  "harness/tsc-violations/tsconfig.json only extends the app config and sets rootDir to itself",
);

const onDisk = ["cases", "support"]
  .flatMap((folder) =>
    readdirSync(join(here, folder)).map((file) => `${folder}/${file}`),
  )
  .sort();
const listed = Object.keys(expected.cases).sort();
report(
  isDeepStrictEqual(onDisk, listed) ? "PASS" : "FAIL",
  `case files match expected.json: on disk ${onDisk.length}, listed ${listed.length}`,
);

for (const compiler of compilers) {
  const run = spawnSync(
    join(desktop, "node_modules", ".bin", compiler),
    ["-p", "tsconfig.json", "--pretty", "false"],
    { cwd: here, encoding: "utf8" },
  );
  const found = new Map();
  for (const line of `${run.stdout}${run.stderr}`.split("\n")) {
    const match = diagnostic.exec(line);
    if (match !== null) {
      const codes = found.get(match[1]) ?? new Set();
      found.set(match[1], codes.add(match[2]));
    } else if (/error TS\d+/.test(line)) {
      report("FAIL", `${compiler}: diagnostic outside a case file: ${line}`);
    }
  }
  for (const file of found.keys()) {
    if (!(file in expected.cases)) {
      report(
        "FAIL",
        `${compiler}: unexpected diagnostics in ${file}: ${[...found.get(file)].join(" ")}`,
      );
    }
  }
  for (const [file, codes] of Object.entries(expected.cases)) {
    const got = [...(found.get(file) ?? [])].sort();
    const want = [...codes].sort();
    report(
      isDeepStrictEqual(got, want) ? "PASS" : "FAIL",
      `${compiler} ${file}: expected [${want.join(" ")}], got [${got.join(" ")}]`,
    );
  }
}

const rootDirProject = join(here, "root-dir");
report(
  isDeepStrictEqual(readJson(join(rootDirProject, "tsconfig.json")), {
    extends: "../../../apps/desktop/tsconfig.json",
    compilerOptions: { rootDir: "src" },
    include: ["src"],
  })
    ? "PASS"
    : "FAIL",
  "harness/tsc-violations/root-dir/tsconfig.json keeps the app rootDir rule for its own src",
);
for (const compiler of compilers) {
  const run = spawnSync(
    join(desktop, "node_modules", ".bin", compiler),
    ["-p", "tsconfig.json", "--pretty", "false"],
    { cwd: rootDirProject, encoding: "utf8" },
  );
  const found = {};
  for (const line of `${run.stdout}${run.stderr}`.split("\n")) {
    const match = diagnostic.exec(line);
    if (match !== null)
      found[match[1]] = [...new Set([...(found[match[1]] ?? []), match[2]])];
  }
  report(
    isDeepStrictEqual(found, expected.rootDirCase) ? "PASS" : "FAIL",
    `${compiler} root-dir: expected ${JSON.stringify(expected.rootDirCase)}, got ${JSON.stringify(found)}`,
  );
}

const moneyProject = join(here, "money");
const moneyExpected = readJson(join(moneyProject, "expected.json"));
report(
  isDeepStrictEqual(readJson(join(moneyProject, "tsconfig.json")), {
    extends: "../../../apps/desktop/tsconfig.json",
    compilerOptions: { rootDir: "../../.." },
    include: ["cases"],
  })
    ? "PASS"
    : "FAIL",
  "harness/tsc-violations/money/tsconfig.json extends the app config and reaches the real money.ts",
);
const moneyOnDisk = readdirSync(join(moneyProject, "cases"))
  .map((file) => `cases/${file}`)
  .sort();
report(
  isDeepStrictEqual(moneyOnDisk, Object.keys(moneyExpected).sort())
    ? "PASS"
    : "FAIL",
  `money case files match money/expected.json: ${moneyOnDisk.length}`,
);
for (const compiler of compilers) {
  const run = spawnSync(
    join(desktop, "node_modules", ".bin", compiler),
    ["-p", "tsconfig.json", "--pretty", "false"],
    { cwd: moneyProject, encoding: "utf8" },
  );
  const found = Object.fromEntries(moneyOnDisk.map((file) => [file, []]));
  for (const line of `${run.stdout}${run.stderr}`.split("\n")) {
    const match = diagnostic.exec(line);
    if (match !== null && match[1] in found)
      found[match[1]] = [...new Set([...found[match[1]], match[2]])].sort();
    else if (/error TS\d+/.test(line))
      report("FAIL", `${compiler} money: diagnostic outside a case: ${line}`);
  }
  report(
    isDeepStrictEqual(found, moneyExpected) ? "PASS" : "FAIL",
    `${compiler} money: expected ${JSON.stringify(moneyExpected)}, got ${JSON.stringify(found)}`,
  );
}

process.stdout.write(`tsc-violations: ${failures.length} failure(s)\n`);
process.exitCode = failures.length === 0 ? 0 : 1;
