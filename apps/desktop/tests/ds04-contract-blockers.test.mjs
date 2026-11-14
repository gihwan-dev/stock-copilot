import assert from "node:assert/strict";
import { test } from "node:test";
import { cpSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
for (const name of ["Badge", "StatusBar"]) {
  test(`DS04 ${name} semantic tone passes the real public and style contracts`, async () => {
    const folder = mkdtempSync("ds04-semantic-control-");
    try {
      cpSync(join(desktopRoot, "src"), join(folder, "src"), {
        recursive: true,
      });
      const project = join(folder, "tsconfig.json");
      writeFileSync(
        project,
        JSON.stringify({
          extends: join(desktopRoot, "tsconfig.json"),
          include: ["src"],
          compilerOptions: { rootDir: "src" },
        }),
      );
      const engine = new ESLint({
        cwd: folder,
        overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
        overrideConfig: [
          {
            settings: { "boundaries/root-path": folder },
            languageOptions: {
              parserOptions: {
                projectService: false,
                project,
                tsconfigRootDir: folder,
              },
            },
          },
        ],
      });
      const [result] = await engine.lintFiles([
        join(folder, `src/shared/ds/${name}.tsx`),
      ]);
      assert.deepEqual(result.messages, []);
    } finally {
      rmSync(folder, { recursive: true, force: true });
    }
  });
}
