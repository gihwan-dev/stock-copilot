import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { desktop, loadCatalog } from "./load.mjs";
import { generatedCatalogFiles, generationProblems } from "./generation.mjs";
const { samples } = await loadCatalog();
if (process.argv.includes("--check")) {
  const problems = await generationProblems(samples);
  problems.forEach((problem) => process.stderr.write(`${problem}\n`));
  process.exitCode = problems.length === 0 ? 0 : 1;
} else
  for (const [file, source] of Object.entries(
    await generatedCatalogFiles(samples),
  ))
    writeFileSync(join(desktop, ".storybook", file), source);
