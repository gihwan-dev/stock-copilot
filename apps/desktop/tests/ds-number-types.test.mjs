import assert from "node:assert/strict";
import test from "node:test";
import { cpSync, writeFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import { mkdtempSync } from "./ds-runtime-environment.mjs";
import { desktopRoot } from "./paths.mjs";
test("DS05 display percentages reject domain ratios probabilities and interchange", () => {
  const folder = mkdtempSync("ds05-display-types-");
  try {
    cpSync(join(desktopRoot, "src"), join(folder, "src"), { recursive: true });
    const project = join(folder, "tsconfig.json"),
      file = join(folder, "src/shared/ds/TypeProbe.tsx");
    writeFileSync(
      project,
      JSON.stringify({
        extends: join(desktopRoot, "tsconfig.json"),
        compilerOptions: { rootDir: "src" },
        include: ["src"],
      }),
    );
    const prefix =
      'import type {ReactElement} from "react";import {Money,Percent,Probability,PriceChange} from "./Numbers";import type {Money as Amount,Price,Ratio,Probability as DomainProbability,PercentDisplay} from "../lib";declare const amount:Amount;declare const price:Price;declare const ratio:Ratio;declare const probability:DomainProbability;declare const percent:PercentDisplay;';
    for (const [expression, passes] of [
      [
        '<><Money value={price}/><Money value={amount} direction="up"/><Percent value={percent}/><Probability value={percent}/><PriceChange amount={amount} rate={percent} direction="up"/></>',
        true,
      ],
      ["<Percent value={ratio}/>", false],
      ["<Probability value={probability}/>", false],
      ['<PriceChange amount={amount} rate={ratio} direction="up"/>', false],
      ["<Percent value={probability}/>", false],
      ["<Probability value={ratio}/>", false],
    ]) {
      writeFileSync(
        file,
        prefix + "export const Probe=():ReactElement=>" + expression + ";",
      );
      const result = spawnSync(
        join(desktopRoot, "node_modules/.bin/tsc"),
        ["-p", project, "--noUnusedLocals", "false"],
        { encoding: "utf8" },
      );
      assert.equal(result.status === 0, passes, result.stdout + result.stderr);
      if (!passes) assert.match(result.stdout, /percentDisplayKey/);
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
