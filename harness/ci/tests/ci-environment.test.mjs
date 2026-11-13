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

test("runner variables, a BASH_ENV and a hostile shell profile do not change the base step's verdict", async () => {
  const { spawnSync } = await import("node:child_process");
  const { chmodSync, readFileSync, rmSync } = await import("node:fs");
  const { join } = await import("node:path");
  const {
    commit,
    createRepository,
    removeRepository,
    runBaseStep,
    scratch,
    write,
  } = await import("./signature-repo.mjs");
  const hostile = scratch("hostile-runner-");
  const made = createRepository();
  const names = [
    "HOME",
    "BASH_ENV",
    "GITHUB_OUTPUT",
    "GITHUB_REPOSITORY",
    "GITHUB_SHA",
    "GITHUB_REF",
    "GITHUB_EVENT_NAME",
    "RUNNER_TEMP",
    "GH_TOKEN",
    "GITHUB_TOKEN",
    "CI",
    "GITHUB_ACTIONS",
  ];
  const saved = Object.fromEntries(
    names.map((name) => [name, process.env[name]]),
  );
  try {
    write(hostile, "bin/gh", "#!/bin/sh\nexit 4\n");
    chmodSync(join(hostile, "bin/gh"), 0o755);
    const prepend = `export PATH=${join(hostile, "bin")}:$PATH\ngh() { exit 4; }\n`;
    write(hostile, ".bashrc", prepend);
    write(hostile, ".bash_profile", prepend);
    write(hostile, "bash-env.sh", prepend);
    write(hostile, "runner-output.txt", "");
    const head = commit(made.directory, "head");
    Object.assign(process.env, {
      HOME: hostile,
      BASH_ENV: join(hostile, "bash-env.sh"),
      GITHUB_OUTPUT: join(hostile, "runner-output.txt"),
      GITHUB_REPOSITORY: "someone/else",
      GITHUB_SHA: head,
      GITHUB_REF: "refs/heads/master",
      GITHUB_EVENT_NAME: "push",
      RUNNER_TEMP: hostile,
      GH_TOKEN: "x",
      GITHUB_TOKEN: "x",
      CI: "true",
      GITHUB_ACTIONS: "true",
    });
    const plain = spawnSync("bash", ["-c", "type -t gh"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      env: process.env,
    });
    assert.equal(
      plain.stdout.trim(),
      "function",
      "the hostile shell setup is effective",
    );
    assert.deepEqual(
      runBaseStep(
        made,
        {
          EVENT_NAME: "push",
          REF: "refs/heads/master",
          BEFORE: made.base,
          HEAD: head,
        },
        [made.base],
      ),
      {
        status: 0,
        output: { tools: made.base, base: made.base, kind: "verified" },
      },
    );
    assert.equal(readFileSync(join(hostile, "runner-output.txt"), "utf8"), "");
  } finally {
    Object.entries(saved).forEach(([name, value]) => {
      if (value === undefined) {
        delete process.env[name];
      } else {
        process.env[name] = value;
      }
    });
    removeRepository(made.directory);
    rmSync(hostile, { recursive: true, force: true });
  }
});
