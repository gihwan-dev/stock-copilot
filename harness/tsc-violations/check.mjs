import { isDeepStrictEqual } from "node:util";
import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

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

const appConfig = readJson(join(desktop, "tsconfig.json"));
report(
  appConfig.extends === undefined ? "PASS" : "FAIL",
  "apps/desktop/tsconfig.json does not extend another config",
);
for (const [option, value] of Object.entries(expected.options)) {
  const actual = appConfig.compilerOptions[option];
  report(
    isDeepStrictEqual(actual, value) ? "PASS" : "FAIL",
    `option ${option}: expected ${JSON.stringify(value)}, got ${JSON.stringify(actual)}`,
  );
}

const ownConfig = readJson(join(here, "tsconfig.json"));
report(
  isDeepStrictEqual(ownConfig, {
    extends: "../../apps/desktop/tsconfig.json",
    include: ["cases", "support"],
  })
    ? "PASS"
    : "FAIL",
  "harness/tsc-violations/tsconfig.json only extends the app config",
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

process.stdout.write(`tsc-violations: ${failures.length} failure(s)\n`);
process.exitCode = failures.length === 0 ? 0 : 1;
