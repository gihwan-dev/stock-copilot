import { storedProblems } from "./records.mjs";
import { execFileSync } from "node:child_process";
import { resolve } from "node:path";
import { baselineProblems } from "./policy.mjs";
const root = resolve(process.argv[2] ?? resolve(import.meta.dirname, "../.."));
const paths = execFileSync(
  "git",
  ["ls-files", "--cached", "--others", "--exclude-standard", "-z"],
  { cwd: root, encoding: "utf8" },
).split("\0");
const problems = [
  ...baselineProblems(paths),
  ...storedProblems(resolve(root, "harness/visual/baselines")),
];
problems.forEach((problem) => process.stderr.write(`${problem}\n`));
process.stdout.write(`visual-baselines: ${problems.length} problems\n`);
if (problems.length !== 0) process.exitCode = 1;
