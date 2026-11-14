import assert from "node:assert/strict";
import test from "node:test";
import { mkdirSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";

import "@base-ui/react/button";
const witnesses = JSON.parse(
  readFileSync(
    new URL(
      "../../../harness/eslint-violations/ds03-base-components.json",
      import.meta.url,
    ),
    "utf8",
  ),
);

for (const { name, imports, jsx, rule, forbidden } of witnesses) {
  test(`DS03 approved internal adapter: ${name}`, async () => {
    const folder = mkdtempSync("ds03-boundary-");
    try {
      const source = join(folder, "src/shared/ds/Probe.tsx");
      mkdirSync(join(source, ".."), { recursive: true });
      const project = join(folder, "tsconfig.json");
      writeFileSync(
        project,
        JSON.stringify({
          extends: join(desktopRoot, "tsconfig.json"),
          compilerOptions: { rootDir: "src" },
          include: ["src"],
        }),
      );
      writeFileSync(
        source,
        `import type { ReactElement } from "react"; ${imports} export const Probe = (): ReactElement => (${jsx});`,
      );
      const typed = spawnSync(
        join(desktopRoot, "node_modules/.bin/tsc"),
        ["-p", project],
        { encoding: "utf8" },
      );
      assert.equal(typed.status, 0, typed.stdout + typed.stderr);
      const lint = new ESLint({
        cwd: folder,
        overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
        overrideConfig: [
          {
            languageOptions: {
              parserOptions: {
                projectService: false,
                project,
                tsconfigRootDir: folder,
              },
            },
            settings: { "boundaries/root-path": folder },
          },
        ],
      });
      const results = await lint.lintFiles([source]);
      assert.deepEqual(
        results.flatMap((result) => result.messages),
        [],
      );
      if (forbidden !== undefined) {
        writeFileSync(
          source,
          `import type {ReactElement} from "react"; export const Probe=():ReactElement=>(${forbidden});`,
        );
        const rejected = await lint.lintFiles([source]);
        assert.ok(
          rejected
            .flatMap((result) => result.messages)
            .some((message) => message.ruleId === rule),
        );
      }
    } finally {
      rmSync(folder, { recursive: true, force: true });
    }
  });
}
