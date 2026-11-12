import {
  compareGenerated,
  copyGenerated,
  generateInto,
  scratchRoot,
} from "./generated.mjs";

const write = process.argv.includes("--write");
const outRoot = scratchRoot(write ? "write" : "check");
const failures = generateInto(outRoot);
if (failures.length === 0 && write) copyGenerated(outRoot);
const problems =
  failures.length > 0 ? failures : write ? [] : compareGenerated(outRoot);
problems.forEach((problem) => process.stderr.write(`FAIL ${problem}\n`));
process.stdout.write(
  `generated-files: ${problems.length} problem(s)${write ? " (written)" : ""}\n`,
);
process.exitCode = problems.length === 0 ? 0 : 1;
