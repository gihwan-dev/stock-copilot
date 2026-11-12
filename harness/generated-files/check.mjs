import {
  compareGenerated,
  copyGenerated,
  fixedPartProblems,
  generateInto,
  scratchRoot,
} from "./generated.mjs";

const write = process.argv.includes("--write");
const outRoot = scratchRoot(write ? "write" : "check");
const failures = generateInto(outRoot);
const problems =
  failures.length > 0
    ? failures
    : [
        ...fixedPartProblems(outRoot),
        ...(write ? [] : compareGenerated(outRoot)),
      ];
if (problems.length === 0 && write) copyGenerated(outRoot);
problems.forEach((problem) => process.stderr.write(`FAIL ${problem}\n`));
process.stdout.write(
  `generated-files: ${problems.length} problem(s)${write ? " (written)" : ""}\n`,
);
process.exitCode = problems.length === 0 ? 0 : 1;
