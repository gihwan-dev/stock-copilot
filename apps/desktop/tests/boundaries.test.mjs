import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { join } from "node:path";
import { test } from "node:test";
import boundaries from "eslint-plugin-boundaries";
import tseslint from "typescript-eslint";
import {
  BOUNDARY_TABLES,
  boundaryPolicies,
  boundaryRules,
  boundarySettings,
} from "../eslint/boundaries.mjs";
import {
  appAllowlists,
  here,
  readCases,
} from "../../../harness/eslint-violations/lint.mjs";
import { messageKey } from "../../../harness/eslint-violations/verdict.mjs";
import { local } from "../eslint/local-plugin.mjs";
import { desktopRoot } from "./paths.mjs";

const { ESLint } = createRequire(join(desktopRoot, "package.json"))("eslint");
const { cases } = readCases();

const documentedDependencies = {
  app: ["feature", "ds", "ipc", "platform", "lib"],
  feature: ["ds", "ipc", "platform", "lib"],
  "feature-ui": ["ds", "ipc", "platform", "lib"],
  "feature-model": ["ds", "ipc", "platform", "lib"],
  ds: ["lib"],
  ipc: ["lib"],
  platform: ["ipc", "lib"],
  lib: [],
};
const elementTypes = Object.keys(documentedDependencies).filter(
  (type) => !type.startsWith("feature-"),
);
const featureParts = "(feature|feature-ui|feature-model)";

const isBoundaryKey = (key) => /^(warning )?boundaries\//.test(key);

const boundaryFiles = Object.entries(cases)
  .filter(([file, spec]) => /\.tsx?$/.test(file) && Array.isArray(spec.expect))
  .map(([file]) => file);

const expectedBoundaryKeys = (file) =>
  (Array.isArray(cases[file].expect) ? cases[file].expect : [])
    .filter(isBoundaryKey)
    .sort();

const boundaryLinter = (tables, rules = boundaryRules(tables)) =>
  new ESLint({
    cwd: here,
    overrideConfigFile: true,
    cache: false,
    overrideConfig: [
      {
        files: ["src/**/*.ts", "src/**/*.tsx"],
        languageOptions: { parser: tseslint.parser },
        linterOptions: { reportUnusedDisableDirectives: "off" },
        plugins: { boundaries },
        settings: boundarySettings(tables, here),
        rules,
      },
    ],
  });

const failingWith = async (tables, rules) => {
  const results = await boundaryLinter(tables, rules).lintFiles(boundaryFiles);
  return results
    .map((result) => ({
      file: result.filePath.slice(here.length + 1),
      keys: [
        ...new Set(
          result.messages
            .map((message) => messageKey(message, {}))
            .filter(isBoundaryKey),
        ),
      ].sort(),
    }))
    .filter(
      ({ file, keys }) =>
        JSON.stringify(keys) !== JSON.stringify(expectedBoundaryKeys(file)),
    )
    .map(({ file }) => file);
};

const withTable = (name, value) => ({ ...BOUNDARY_TABLES, [name]: value });

test("the boundary tables hold exactly the folder table of architecture 5", () => {
  assert.deepEqual(
    BOUNDARY_TABLES.ELEMENT_DEPENDENCIES,
    documentedDependencies,
  );
  assert.deepEqual(
    BOUNDARY_TABLES.ELEMENTS.map(({ type, pattern, partialMatch }) => [
      type,
      pattern,
      partialMatch,
    ]),
    [
      ["app", "src/app", false],
      ["feature-ui", "src/features/*/ui", false],
      ["feature-model", "src/features/*/model", false],
      ["feature", "src/features/*", false],
      ["ds", "src/shared/ds", false],
      ["ipc", "src/shared/ipc", false],
      ["platform", "src/shared/platform", false],
      ["lib", "src/shared/lib", false],
    ],
  );
  assert.equal(BOUNDARY_TABLES.ENTRY_FILE, "index.ts");
  assert.deepEqual(BOUNDARY_TABLES.FEATURE_ENTRIES, [
    "ui",
    "model",
    "index.ts",
    "messages.ts",
  ]);
  assert.deepEqual(BOUNDARY_TABLES.FEATURE_DEPENDENCIES, {
    feature: { "feature-ui": "**", "feature-model": "**" },
    "feature-ui": { "feature-model": "**", feature: "messages.ts" },
    "feature-model": {},
  });
  assert.deepEqual(BOUNDARY_TABLES.PACKAGE_LOCATIONS, {
    "react-dom": ["src/shared/platform/index.ts"],
    "@tauri-apps/api": ["src/shared/ipc/**"],
    zod: ["src/shared/ipc/**", "src/shared/lib/brand.ts"],
  });
});

test("the zod location in shared/lib is a file of the raw type allowlist", () => {
  const libFiles = BOUNDARY_TABLES.PACKAGE_LOCATIONS.zod.filter((path) =>
    path.startsWith("src/shared/lib/"),
  );
  assert.deepEqual(
    libFiles.filter((path) => !appAllowlists.RAW_TYPE_ALLOWLIST.includes(path)),
    [],
  );
});

test("every forbidden direction of the folder table has a failing case", () => {
  const expected = new Set(Object.values(cases).flatMap((spec) => spec.expect));
  const missing = elementTypes.flatMap((from) =>
    elementTypes
      .filter(
        (to) =>
          !documentedDependencies[from].includes(to) &&
          (from !== to || from === "feature"),
      )
      .filter(
        (to) =>
          ![...expected].some(
            (key) =>
              typeof key === "string" &&
              new RegExp(
                `^boundaries/dependencies :: ${from === "feature" ? featureParts : from}에서 ${to}의 index\\.ts `,
              ).test(key),
          ),
      )
      .map((to) => `${from} -> ${to}`),
  );
  assert.deepEqual(missing, []);
});

test("inside a feature only ui -> model, ui -> messages.ts and index.ts -> ui, model pass", () => {
  const expected = new Set(
    Object.values(cases).flatMap((spec) =>
      Array.isArray(spec.expect) ? spec.expect : [],
    ),
  );
  const forbidden = [
    "feature-model에서 feature-ui의 ",
    "feature-model에서 feature의 messages.ts ",
    "feature-ui에서 feature의 index.ts ",
    "feature-ui에서 feature-model의 ",
  ];
  assert.deepEqual(
    forbidden.filter(
      (part) =>
        ![...expected].some((key) =>
          key.startsWith(`boundaries/dependencies :: ${part}`),
        ),
    ),
    [],
  );
  [
    "src/features/boundary/ui/to-model.ts",
    "src/features/boundary/ui/to-messages.ts",
    "src/features/boundary/index.ts",
  ].forEach((file) => assert.deepEqual(cases[file].expect, [], file));
});

test("the light boundary linter reproduces the boundary keys of every case", async () => {
  assert.ok(boundaryFiles.length > 50);
  assert.deepEqual(await failingWith(BOUNDARY_TABLES), []);
});

test("removing any allowed direction, element, package location or the entry file makes some case fail", async () => {
  const variants = [
    ...Object.entries(BOUNDARY_TABLES.ELEMENT_DEPENDENCIES).flatMap(
      ([from, targets]) =>
        targets.map((to) => ({
          label: `${from} -> ${to}`,
          tables: withTable("ELEMENT_DEPENDENCIES", {
            ...BOUNDARY_TABLES.ELEMENT_DEPENDENCIES,
            [from]: targets.filter((other) => other !== to),
          }),
        })),
    ),
    ...Object.entries(BOUNDARY_TABLES.FEATURE_DEPENDENCIES).flatMap(
      ([from, targets]) =>
        Object.keys(targets).map((to) => ({
          label: `inside a feature ${from} -> ${to}`,
          tables: withTable("FEATURE_DEPENDENCIES", {
            ...BOUNDARY_TABLES.FEATURE_DEPENDENCIES,
            [from]: Object.fromEntries(
              Object.entries(targets).filter(([other]) => other !== to),
            ),
          }),
        })),
    ),
    {
      label: "zod widened to every shared/lib file",
      tables: withTable("PACKAGE_LOCATIONS", {
        ...BOUNDARY_TABLES.PACKAGE_LOCATIONS,
        zod: ["src/shared/ipc/**", "src/shared/lib/*.ts"],
      }),
    },
    {
      label: "react-dom widened to every app file",
      tables: withTable("PACKAGE_LOCATIONS", {
        ...BOUNDARY_TABLES.PACKAGE_LOCATIONS,
        "react-dom": ["src/app/**"],
      }),
    },
    ...BOUNDARY_TABLES.ELEMENTS.map((element) => ({
      label: `element ${element.type}`,
      tables: withTable(
        "ELEMENTS",
        BOUNDARY_TABLES.ELEMENTS.filter((other) => other !== element),
      ),
    })),
    ...BOUNDARY_TABLES.ELEMENTS.map((element) => ({
      label: `partialMatch of ${element.type}`,
      tables: withTable(
        "ELEMENTS",
        BOUNDARY_TABLES.ELEMENTS.map((other) =>
          other === element ? { ...other, partialMatch: true } : other,
        ),
      ),
    })),
    ...Object.entries(BOUNDARY_TABLES.PACKAGE_LOCATIONS).flatMap(
      ([source, paths]) => [
        {
          label: `package ${source}`,
          tables: withTable(
            "PACKAGE_LOCATIONS",
            Object.fromEntries(
              Object.entries(BOUNDARY_TABLES.PACKAGE_LOCATIONS).filter(
                ([other]) => other !== source,
              ),
            ),
          ),
        },
        ...paths.map((path) => ({
          label: `${source} at ${path}`,
          tables: withTable("PACKAGE_LOCATIONS", {
            ...BOUNDARY_TABLES.PACKAGE_LOCATIONS,
            [source]: paths.filter((other) => other !== path),
          }),
        })),
      ],
    ),
    { label: "entry file", tables: withTable("ENTRY_FILE", "**") },
  ];
  const survivors = [];
  for (const { label, tables } of variants)
    if ((await failingWith(tables)).length === 0) survivors.push(label);
  assert.deepEqual(survivors, []);
});

test("removing any policy or rule option, or lowering any boundary rule, makes some case fail", async () => {
  const rules = boundaryRules(BOUNDARY_TABLES);
  const [severity, options] = rules["boundaries/dependencies"];
  const policies = boundaryPolicies(BOUNDARY_TABLES);
  const variants = [
    ...policies.map((policy, index) => ({
      label: `policy ${index}`,
      rules: {
        ...rules,
        "boundaries/dependencies": [
          severity,
          {
            ...options,
            policies: policies.filter((other) => other !== policy),
          },
        ],
      },
    })),
    ...[
      "checkAllOrigins",
      "checkUnknownLocals",
      "checkInternals",
      "message",
    ].map((key) => ({
      label: `option ${key}`,
      rules: {
        ...rules,
        "boundaries/dependencies": [
          severity,
          Object.fromEntries(
            Object.entries(options).filter(([other]) => other !== key),
          ),
        ],
      },
    })),
    {
      label: "default allow",
      rules: {
        ...rules,
        "boundaries/dependencies": [severity, { ...options, default: "allow" }],
      },
    },
    ...Object.keys(rules).flatMap((rule) =>
      ["off", "warn"].map((level) => ({
        label: `${rule} ${level}`,
        rules: {
          ...rules,
          [rule]: Array.isArray(rules[rule])
            ? [level, ...rules[rule].slice(1)]
            : level,
        },
      })),
    ),
  ];
  const survivors = [];
  for (const { label, rules: variant } of variants)
    if ((await failingWith(BOUNDARY_TABLES, variant)).length === 0)
      survivors.push(label);
  assert.deepEqual(survivors, []);
});

const deprecatedRules = [
  "boundaries/element-types",
  "boundaries/entry-point",
  "boundaries/external",
  "boundaries/no-private",
  "boundaries/no-unknown",
  "boundaries/no-ignored",
];

test("only the three v7 boundary rules are configured, as errors, for app files", async () => {
  const app = new ESLint({ cwd: desktopRoot, cache: false });
  for (const file of [
    "src/app/main.tsx",
    "src/features/home/ui/HomeScreen.tsx",
    "src/shared/lib/result.ts",
    "src/shared/ipc/testing/mocks.ts",
  ]) {
    const config = await app.calculateConfigForFile(join(desktopRoot, file));
    const configured = Object.keys(config.rules)
      .filter((rule) => rule.startsWith("boundaries/"))
      .sort();
    assert.deepEqual(
      configured,
      [
        "boundaries/dependencies",
        "boundaries/no-unknown-dependencies",
        "boundaries/no-unknown-files",
      ],
      file,
    );
    configured.forEach((rule) =>
      assert.equal([config.rules[rule]].flat()[0], 2, `${file} ${rule}`),
    );
    assert.deepEqual(
      deprecatedRules.filter((rule) => rule in config.rules),
      [],
    );
    assert.equal(
      config.settings["boundaries/root-path"],
      desktopRoot,
      `${file} root path`,
    );
  }
});

const eslintBin = join(desktopRoot, "node_modules", ".bin", "eslint");

test("linting src prints nothing, so no console-only boundaries warning hides behind --max-warnings 0 (S-19)", () => {
  const run = spawnSync(eslintBin, ["--max-warnings", "0", "src"], {
    cwd: desktopRoot,
    encoding: "utf8",
  });
  assert.equal(run.status, 0, run.stdout + run.stderr);
  assert.equal(run.stdout, "");
  assert.equal(run.stderr, "");
});

test("a deprecated boundaries rule only prints a console warning and passes --max-warnings 0 (S-19)", () => {
  const script = `
    import { createRequire } from "node:module";
    const { ESLint } = createRequire(${JSON.stringify(join(desktopRoot, "package.json"))})("eslint");
    const eslint = new ESLint({ cwd: ${JSON.stringify(desktopRoot)}, cache: false, overrideConfig: { files: ["src/**/*.ts", "src/**/*.tsx"], rules: { "boundaries/no-unknown": "error" } } });
    const results = await eslint.lintFiles(["src"]);
    process.stdout.write("warnings=" + results.reduce((n, r) => n + r.warningCount + r.errorCount, 0) + "\\n");
  `;
  const run = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", script],
    {
      cwd: desktopRoot,
      encoding: "utf8",
    },
  );
  assert.equal(run.status, 0, run.stderr);
  assert.match(
    run.stdout + run.stderr,
    /\[boundaries\]\[warning\]: Rule name "boundaries\/no-unknown" is deprecated/,
  );
  assert.match(run.stdout, /^warnings=0$/m);
});

const layoutMessage = local.rules["forbidden-file"].meta.messages.featureLayout;

const layoutFailures = async (featureEntries) => {
  const linter = new ESLint({
    cwd: here,
    overrideConfigFile: true,
    cache: false,
    overrideConfig: [
      {
        files: ["src/**/*.ts", "src/**/*.tsx"],
        languageOptions: { parser: tseslint.parser },
        linterOptions: { reportUnusedDisableDirectives: "off" },
        plugins: { local },
        rules: {
          "local/forbidden-file": [
            "error",
            {
              checkExtension: false,
              placeholders: [],
              folders: [],
              featureEntries,
            },
          ],
        },
      },
    ],
  });
  const results = await linter.lintFiles(boundaryFiles);
  return results
    .filter(
      (result) =>
        result.messages.some((message) => message.message === layoutMessage) !==
        cases[result.filePath.slice(here.length + 1)].expect.includes(
          `local/forbidden-file :: ${layoutMessage}`,
        ),
    )
    .map((result) => result.filePath.slice(here.length + 1));
};

test("the feature layout check reproduces the cases and every allowed entry is needed", async () => {
  const entries = BOUNDARY_TABLES.FEATURE_ENTRIES;
  assert.deepEqual(await layoutFailures(entries), []);
  const survivors = [];
  for (const entry of entries)
    if (
      (await layoutFailures(entries.filter((other) => other !== entry)))
        .length === 0
    )
      survivors.push(entry);
  assert.deepEqual(survivors, []);
});
