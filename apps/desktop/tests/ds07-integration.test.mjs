import assert from "node:assert/strict";
import test from "node:test";
import {
  cpSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
  existsSync,
  unlinkSync,
  symlinkSync,
  rmSync,
} from "node:fs";
import { mkdtempSync } from "../../../harness/ci/test-temp.mjs";
import { join, basename } from "node:path";
import { spawnSync } from "node:child_process";
import { repoRoot } from "./paths.mjs";
const childEnvironment = Object.fromEntries(
  Object.entries(process.env).filter(([key]) => key !== "NODE_TEST_CONTEXT"),
);
const cases = JSON.parse(
  readFileSync(join(repoRoot, "harness/ds-catalog/cases.json"), "utf8"),
);
test("DS07 actual Storybook index and CSF mutations fail the registered checker", () => {
  mkdirSync(join(repoRoot, "target"), { recursive: true });
  const scratch = mkdtempSync(join(repoRoot, "target/DS-07-integration-"));
  try {
    for (const path of [
      "package.json",
      "pnpm-workspace.yaml",
      "pnpm-lock.yaml",
      "apps/desktop",
      "harness/ds-catalog",
      "harness/ci",
      "design/fonts",
    ])
      cpSync(join(repoRoot, path), join(scratch, path), {
        recursive: true,
        filter: (path) =>
          !["node_modules", "dist", "src-tauri", "e2e-app"].includes(
            basename(path),
          ),
      });
    symlinkSync(
      join(repoRoot, "node_modules"),
      join(scratch, "node_modules"),
      "dir",
    );
    symlinkSync(
      join(repoRoot, "apps/desktop/node_modules"),
      join(scratch, "apps/desktop/node_modules"),
      "dir",
    );
    const run = () =>
      spawnSync(process.execPath, ["harness/ds-catalog/check.mjs"], {
        cwd: scratch,
        env: childEnvironment,
        encoding: "utf8",
        maxBuffer: 16 * 1024 * 1024,
      });
    const baseline = run();
    assert.equal(baseline.status, 0, baseline.stdout + baseline.stderr);
    for (const item of cases) {
      const path = join(scratch, item.file),
        original = existsSync(path) ? readFileSync(path, "utf8") : undefined;
      if (item.from !== undefined)
        assert.ok(original.includes(item.from), item.name);
      writeFileSync(
        path,
        (item.prepend ?? "") +
          (item.create ??
            (item.append === undefined
              ? original.replace(item.from, item.to)
              : original + item.append)),
      );
      assert.equal(
        (item.compile === "typescript"
          ? spawnSync(
              join(scratch, "apps/desktop/node_modules/.bin/tsc"),
              ["-p", join(scratch, "apps/desktop/tsconfig.json")],
              { cwd: scratch, env: childEnvironment, encoding: "utf8" },
            )
          : spawnSync(process.execPath, ["--check", path])
        ).status,
        0,
        item.name,
      );
      const result =
        item.command === undefined
          ? run()
          : spawnSync(process.execPath, item.command, {
              cwd: scratch,
              env: childEnvironment,
              encoding: "utf8",
              maxBuffer: 16 * 1024 * 1024,
            });
      assert.notEqual(
        result.status,
        0,
        `${item.name}\n${result.stdout}\n${result.stderr}`,
      );
      if (item.expected !== undefined)
        assert.ok(
          (result.stdout + result.stderr).includes(item.expected),
          item.name,
        );
      if (original === undefined) unlinkSync(path);
      else writeFileSync(path, original);
    }
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }
});
