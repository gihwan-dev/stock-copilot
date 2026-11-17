import { strict as assert } from "node:assert";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { repoRoot } from "./paths.mjs";

test("full verification runs the fixed-browser overflow examples", () => {
  const source = readFileSync(join(repoRoot, "justfile"), "utf8");
  const full =
    source.match(/^check-full:[^\n]*\n(?: {4}[^\n]*\n)+/mu)?.[0] ?? "";
  assert.match(full, /^ {4}node harness\/ds-overflow\/examples\.mjs$/mu);
});

test("regular verification type-checks the browser test entry points", () => {
  const source = readFileSync(join(repoRoot, "justfile"), "utf8");
  const check =
    source.match(/^check-ts:[^\n]*\n(?: {4}[^\n]*\n)+/mu)?.[0] ?? "";
  assert.match(check, /^ {4}node harness\/ds-overflow\/typecheck\.mjs$/mu);
});

test("browser typecheck rejects an invalid browser-test type", async () => {
  const { browserTypeErrors } =
    await import("../../../harness/ds-overflow/typecheck.mjs");
  mkdirSync(join(repoRoot, "target/DS-20"), { recursive: true });
  const folder = mkdtempSync(join(repoRoot, "target/DS-20/typecheck-"));
  try {
    const file = join(folder, "invalid.browser.ts");
    writeFileSync(file, 'export const value: number = "not a number";\n');
    assert.ok(
      browserTypeErrors([file]).some(
        (error) => error.code === 2322 && error.file?.fileName === file,
      ),
    );
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});

test("browser test lint rejects skipped or empty test tables", async () => {
  const { ESLint } = await import("eslint");
  const engine = new ESLint({
    cwd: repoRoot,
    overrideConfigFile: join(
      repoRoot,
      "apps/desktop/eslint.scripts.config.mjs",
    ),
  });
  for (const code of [
    'it.skip("case", () => expect(1).toBe(1));',
    'it.each([])("case", () => expect(1).toBe(1));',
  ]) {
    const [result] = await engine.lintText(code, {
      filePath: "apps/desktop/tests/ds20-overflow.browser.ts",
    });
    assert.ok(
      result.messages.some(
        (message) => message.ruleId === "guard/no-test-skip",
      ),
      code,
    );
  }
});
