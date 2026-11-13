import { strict as assert } from "node:assert";
import { pathToFileURL } from "node:url";
import { test } from "node:test";
import {
  appConfigFile,
  readCases,
  runCase,
  vitestEnvironment,
  withConfig,
} from "../../vitest-check/vitest.mjs";

const FOCUSED_CASE = "only";

const CI_VARIABLES = {
  CI: "true",
  GITHUB_ACTIONS: "true",
  CONTINUOUS_INTEGRATION: "true",
  BUILD_NUMBER: "1",
  RUN_ID: "1",
  VITEST_MODE: "run",
  NODE_OPTIONS: "--require ./x.cjs",
  NODE_ENV: "production",
  TEST: "true",
};

const withCiVariables = (action) => {
  const saved = Object.fromEntries(
    Object.keys(CI_VARIABLES).map((name) => [name, process.env[name]]),
  );
  Object.assign(process.env, CI_VARIABLES);
  try {
    return action();
  } finally {
    Object.entries(saved).forEach(([name, value]) => {
      if (value === undefined) {
        delete process.env[name];
      } else {
        process.env[name] = value;
      }
    });
  }
};

test("Vitest runs with only the allowlisted environment, whatever the CI runner sets", () => {
  assert.deepEqual(
    vitestEnvironment({ ...CI_VARIABLES, PATH: "/bin", HOME: "/home/x" }),
    { PATH: "/bin", HOME: "/home/x" },
  );
});

test("with CI variables set, dropping allowOnly still changes the only case and the full config does not", async () => {
  const appConfig = (await import(pathToFileURL(appConfigFile).href)).default;
  const expected = [...readCases().cases[FOCUSED_CASE]].sort();
  const withoutAllowOnly = {
    ...appConfig,
    test: Object.fromEntries(
      Object.entries(appConfig.test).filter(([key]) => key !== "allowOnly"),
    ),
  };
  withCiVariables(() => {
    assert.deepEqual(runCase(FOCUSED_CASE), expected);
    assert.notDeepEqual(
      withConfig(withoutAllowOnly, (file) => runCase(FOCUSED_CASE, file)),
      expected,
    );
  });
});

test("CI installs the pinned pnpm as corepack's global default before any pnpm step", async () => {
  const { pinnedPackageManager, pnpmSetupCommands } =
    await import("../tools.mjs");
  const { readFileSync } = await import("node:fs");
  const { join } = await import("node:path");
  const root = join(import.meta.dirname, "..", "..", "..");
  const pinned = pinnedPackageManager(root);
  assert.match(pinned, /^pnpm@\d+\.\d+\.\d+\+sha512\.[0-9a-f]{128}$/u);
  assert.deepEqual(pnpmSetupCommands(pinned), [
    ["corepack", ["enable"]],
    ["corepack", ["install", "--global", pinned]],
  ]);
  ["ci.yml", "nightly.yml"].forEach((name) => {
    const workflow = JSON.parse(
      readFileSync(join(root, ".github", "workflows", name), "utf8"),
    );
    Object.entries(workflow.jobs)
      .filter(([, job]) =>
        job.steps.some((step) =>
          /pnpm|just |tools\.mjs fetch/u.test(step.run ?? ""),
        ),
      )
      .forEach(([job, { steps }]) => {
        const setup = steps.findIndex(
          (step) => step.run === "node harness/ci/tools.mjs pnpm",
        );
        const first = steps.findIndex(
          (step) =>
            step.run !== undefined &&
            step.run !== "node harness/ci/tools.mjs pnpm" &&
            /pnpm|^just |tools\.mjs (fetch|cargo-install)/u.test(step.run),
        );
        assert.ok(setup !== -1 && setup < first, `${name} ${job}`);
      });
  });
});
