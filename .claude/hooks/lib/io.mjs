import { readFileSync } from "node:fs";

export const readInput = () => JSON.parse(readFileSync(0, "utf8"));

export const writeOutput = (output) => {
  if (output !== undefined) {
    process.stdout.write(`${JSON.stringify(output)}\n`);
  }
};
