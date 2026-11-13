import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync, readdirSync } from "node:fs";
import { ESLint } from "eslint";
import { dsConfigs, DS_RULES, SCREEN_RULES } from "../eslint/ds-config.mjs";
import {
  cssBoundaryProblems,
  snapshot,
  collectRules,
  generationProblems,
} from "../../../harness/ds-style/check.mjs";
import { desktopRoot, repoRoot } from "./paths.mjs";

const read = (p) => readFileSync(`${repoRoot}/${p}`, "utf8");
const cases = JSON.parse(read("harness/ds-style/cases.json"));
const lint = async (code, rules = DS_RULES) => {
  const engine = new ESLint({
    cwd: desktopRoot,
    overrideConfigFile: true,
    overrideConfig: dsConfigs(rules),
  });
  const [result] = await engine.lintText(code, {
    filePath: "src/shared/ds/Probe.tsx",
  });
  return [...new Set(result.messages.map((m) => m.ruleId))].sort();
};

test("DS02 examples match exact rule sets and disabling each enforcing rule loses its witness", async () => {
  assert.equal(Object.keys(cases).length, 110);
  assert.deepEqual(
    readdirSync(`${repoRoot}/harness/ds-style/src/shared/ds`).sort(),
    Object.keys(cases).sort(),
  );
  for (const [file, spec] of Object.entries(cases)) {
    const code = read(`harness/ds-style/src/shared/ds/${file}`);
    assert.deepEqual(await lint(code), [...spec.expect].sort(), file);
    for (const rule of spec.expect) {
      const isolated = Object.fromEntries(
        Object.keys(DS_RULES).map((key) => [
          key,
          key === rule ? DS_RULES[key] : "off",
        ]),
      );
      assert.deepEqual(
        await lint(code, isolated),
        [rule],
        `${file}: enabled ${rule}`,
      );
      assert.deepEqual(
        await lint(code, { ...isolated, [rule]: "off" }),
        [],
        `${file}: disabled ${rule}`,
      );
    }
  }
});

test("DS02 CSS boundary only permits generated tokens and the fixed theme", () => {
  assert.deepEqual(
    cssBoundaryProblems(
      "theme.css",
      read("apps/desktop/src/shared/ds/theme.css"),
    ),
    [],
  );
  assert.deepEqual(cssBoundaryProblems("theme.css", "@plugin custom;"), [
    "theme entry changed",
  ]);
  for (const value of [
    "rgb()",
    "rotate()",
    "theme()",
    "attr()",
    "image-set('http://x')",
    "!important",
    "@layer",
    "@source inline",
    "@config",
    "@plugin",
    "@custom-variant",
    "@media (min-width:13px)",
    "VAR(--x)",
  ])
    assert.deepEqual(cssBoundaryProblems("custom.css", value), [
      "handwritten CSS forbidden",
    ]);
});

test("DS02 actual Tailwind selector snapshot is stable and reset removal changes it", async () => {
  assert.deepEqual(
    await snapshot(),
    JSON.parse(read("harness/ds-style/utilities.json")),
  );
  assert.notDeepEqual(await snapshot("without-reset"), await snapshot());
});

test("DS02 fixes plugin cwd and entryPoint in the effective application config", async () => {
  const { DS_ENTRY } = await import("../eslint/ds-config.mjs");
  assert.equal(DS_ENTRY, `${desktopRoot}/src/shared/ds/theme.css`);
  const engine = new ESLint({ cwd: desktopRoot });
  const config = await engine.calculateConfigForFile("src/shared/ds/Probe.tsx");
  assert.deepEqual(config.settings["better-tailwindcss"], {
    cwd: `${desktopRoot}/`,
    entryPoint: DS_ENTRY,
  });
  for (const [rule, value] of Object.entries(DS_RULES))
    assert.deepEqual(
      config.rules[rule],
      Array.isArray(value) ? [2, ...value.slice(1)] : [2],
    );
  const screen = await engine.calculateConfigForFile(
    "src/features/home/ui/Probe.tsx",
  );
  for (const rule of Object.keys(DS_RULES))
    assert.deepEqual(
      screen.rules[rule],
      rule in SCREEN_RULES ? [2] : undefined,
    );
});

test("DS02 wrong entryPoint fixtures expose fail-open default classes", async () => {
  const { mkdtempSync, writeFileSync, rmSync } = await import("node:fs");
  const folder = mkdtempSync(`${desktopRoot}/.ds-02-entry-`);
  try {
    writeFileSync(
      `${folder}/without-reset.css`,
      `@import "${desktopRoot}/node_modules/tailwindcss/index.css";\n`,
    );
    const unknown = { "better-tailwindcss/no-unknown-classes": "error" };
    const run = async (entryPoint) => {
      const configs = dsConfigs(
        Object.fromEntries(
          Object.keys(DS_RULES).map((key) => [key, unknown[key] ?? "off"]),
        ),
      );
      configs[0].settings["better-tailwindcss"].entryPoint = entryPoint;
      const engine = new ESLint({
        cwd: desktopRoot,
        overrideConfigFile: true,
        overrideConfig: configs,
      });
      const [result] = await engine.lintText(
        'const probe = <div className="bg-red-500" />;',
        { filePath: "src/shared/ds/Entry.tsx" },
      );
      return result.messages.map((m) => m.ruleId);
    };
    const { DS_ENTRY } = await import("../eslint/ds-config.mjs");
    assert.deepEqual(await run(DS_ENTRY), [
      "better-tailwindcss/no-unknown-classes",
    ]);
    for (const entry of [
      `${folder}/missing.css`,
      `${desktopRoot}/src/shared/ds/tokens.css`,
      `${folder}/without-reset.css`,
    ]) {
      const result = await run(entry);
      process.stdout.write(
        `entryPoint ${entry.split("/").at(-1)}: ${JSON.stringify(result)}\n`,
      );
      assert.deepEqual(
        result,
        entry.endsWith("/tokens.css")
          ? ["better-tailwindcss/no-unknown-classes"]
          : [],
        entry,
      );
      assert.notEqual(entry, DS_ENTRY);
    }
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});

test("DS02 snapshot includes nested variants space utilities and ungenerated candidates", async () => {
  const { DS_CLASSES, DS_VARIANTS } = await import("../eslint/ds-classes.mjs");
  const baseline = await snapshot();
  assert.deepEqual(generationProblems(baseline), []);
  for (const prefix of [
    "hover:",
    "motion-reduce:",
    "window-md:",
    "@content-medium:",
    "space-x-",
  ])
    assert.ok(
      baseline.candidates.some(
        (r) => r.name.startsWith(prefix) && r.generated === "yes",
      ),
      prefix,
    );
  assert.ok(
    baseline.rules.some((r) => r.context.some((c) => c.startsWith("@media"))),
  );
  assert.ok(
    baseline.rules.some((r) =>
      r.context.some((c) => c.startsWith("@container")),
    ),
  );
  for (const variant of ["print", "dark", "group-hover"])
    assert.notDeepEqual(
      await snapshot("normal", DS_CLASSES, [...DS_VARIANTS, variant]),
      baseline,
      variant,
    );
  for (const name of ["min-h-0", "p-0", "animate-spin"]) {
    const changed = await snapshot("normal", [...DS_CLASSES, name]);
    assert.notDeepEqual(changed, baseline, name);
    assert.ok(
      generationProblems(changed).some((p) => p.endsWith(`: ${name}`)),
      name,
    );
  }
  assert.deepEqual(
    collectRules(
      "@media print { @container (width > 1px) { .x { color:red } } }",
    ),
    [{ selector: ".x", context: ["@media print", "@container (width > 1px)"] }],
  );
});
