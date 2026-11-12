import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  fixtureExpectations,
  fixtureNames,
  linksUnder,
  withFixture,
} from "../../../harness/source-tree/tree.mjs";
import { repoRoot } from "./paths.mjs";

const justfile = readFileSync(join(repoRoot, "justfile"), "utf8");

test("every symbolic link under a source tree is found, files and folders alike", () => {
  assert.deepEqual(
    fixtureNames.filter(
      (name) =>
        JSON.stringify(withFixture(name, linksUnder)) !==
        JSON.stringify(fixtureExpectations[name]),
    ),
    [],
  );
  assert.ok(
    fixtureNames.filter((name) => fixtureExpectations[name].length > 0)
      .length >= 3,
  );
});

test("just check scans src for symbolic links before ESLint walks it", () => {
  const lines = justfile.split("\n");
  const scan = lines.indexOf("    node harness/source-tree/check.mjs");
  const lint = lines.indexOf(
    "    pnpm -C apps/desktop exec eslint --max-warnings 0 src",
  );
  assert.ok(scan > 0 && lint > scan);
});

const probe =
  "probe:\n    printf '%s' \"${ESLINT_PLUGIN_BOUNDARIES_ROOT_PATH:-unset}\"\n";

const probeOutput = (file) =>
  spawnSync("just", ["--justfile", file, "probe"], {
    encoding: "utf8",
    env: { ...process.env, ESLINT_PLUGIN_BOUNDARIES_ROOT_PATH: "/tmp/hostile" },
  }).stdout;

test("the boundaries root path variable does not reach just recipes", () => {
  const directory = mkdtempSync(join(tmpdir(), "boundaries-env-"));
  const fixed = join(directory, "fixed.just");
  const plain = join(directory, "plain.just");
  writeFileSync(fixed, `import '${join(repoRoot, "justfile")}'\n\n${probe}`);
  writeFileSync(plain, probe);
  const results = [probeOutput(fixed), probeOutput(plain)];
  rmSync(directory, { recursive: true, force: true });
  assert.deepEqual(results, ["unset", "/tmp/hostile"]);
});
