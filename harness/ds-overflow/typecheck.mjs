import { createRequire } from "node:module";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(import.meta.dirname, "../..");
const desktop = join(root, "apps/desktop");
const require = createRequire(join(desktop, "package.json"));
const ts = require("typescript");
const configPath = join(desktop, "tsconfig.json");
const read = ts.readConfigFile(configPath, ts.sys.readFile);
if (read.error !== undefined)
  throw new Error(
    ts.flattenDiagnosticMessageText(read.error.messageText, "\n"),
  );
const parsed = ts.parseJsonConfigFileContent(read.config, ts.sys, desktop);
const testFiles = [
  "tests/ds20-overflow.browser.ts",
  "tests/ds20-catalog.browser.ts",
  "tests/ds20-browser-commands.d.ts",
].map((file) => join(desktop, file));
const options = {
  ...parsed.options,
  rootDir: root,
  types: ["vitest/globals"],
};
const host = {
  ...ts.createCompilerHost(options),
  getCurrentDirectory: () => desktop,
};
export const browserTypeErrors = (extraFiles = []) => {
  const program = ts.createProgram(
    [...parsed.fileNames, ...testFiles, ...extraFiles],
    options,
    host,
  );
  return [...parsed.errors, ...ts.getPreEmitDiagnostics(program)];
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const errors = browserTypeErrors();
  if (errors.length !== 0) {
    process.stderr.write(
      ts.formatDiagnosticsWithColorAndContext(errors, {
        getCanonicalFileName: (file) => file,
        getCurrentDirectory: () => root,
        getNewLine: () => "\n",
      }),
    );
    process.exitCode = 1;
  } else {
    process.stdout.write("overflow-browser types: 0 errors\n");
  }
}
