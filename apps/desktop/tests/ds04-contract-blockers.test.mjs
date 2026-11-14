import assert from "node:assert/strict";
import { test } from "node:test";
import { cpSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { ESLint, mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
for (const [name, tones] of [
  ["Badge", '"neutral" | "primary" | "up" | "down" | "alert"'],
  ["StatusBar", '"warning" | "error" | "info"'],
]) {
  test(`DS04 ${name} design tone needs a public contract policy decision`, async () => {
    const folder = mkdtempSync("ds04-blocker-");
    try {
      cpSync(join(desktopRoot, "src"), join(folder, "src"), {
        recursive: true,
      });
      writeFileSync(
        join(folder, "tsconfig.json"),
        JSON.stringify({
          extends: join(desktopRoot, "tsconfig.json"),
          include: ["src"],
          compilerOptions: { rootDir: "src" },
        }),
      );
      const file = join(folder, "src/shared/ds/Blocker.tsx");
      writeFileSync(
        file,
        `import type {ReactElement} from "react";import type {UiText} from "../lib";type Props={readonly tone?:${tones};readonly children:UiText};export const ${name}=(props:Props):ReactElement=><span>{props.children}</span>;`,
      );
      const engine = new ESLint({
        cwd: folder,
        overrideConfigFile: join(desktopRoot, "eslint.config.mjs"),
      });
      const [result] = await engine.lintFiles([file]);
      assert.deepEqual(
        result.messages
          .filter((message) => message.ruleId === "ds/public-props")
          .map((message) => message.ruleId),
        ["ds/public-props"],
      );
    } finally {
      rmSync(folder, { recursive: true, force: true });
    }
  });
}
