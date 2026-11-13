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
