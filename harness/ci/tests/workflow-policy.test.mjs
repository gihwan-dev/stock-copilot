import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { installActionSpec, readTools } from "../tools.mjs";
import {
  POLICY_FILE,
  RULES,
  readPolicy,
  repositoryFindings,
  workflowFindings,
} from "../workflow-policy/check.mjs";
import { repoRoot, scratch } from "./signature-repo.mjs";

const CASES_FILE = "harness/ci/workflow-policy/cases.json";

const policy = readPolicy(repoRoot);

const toolSpec = installActionSpec(readTools(repoRoot));

const cases = JSON.parse(
  readFileSync(join(repoRoot, CASES_FILE), "utf8"),
).cases;

const caseResults = (candidate) =>
  cases.map((entry) => ({
    name: entry.name,
    rules: workflowFindings(
      entry.path,
      readFileSync(join(repoRoot, entry.example), "utf8"),
      candidate,
      toolSpec,
    ),
  }));

const matchesCases = (candidate) =>
  caseResults(candidate).every(
    (result, index) =>
      JSON.stringify(result.rules) === JSON.stringify(cases[index].rules),
  );

test("every workflow policy case reports exactly its rules", () => {
  caseResults(policy).forEach((result, index) =>
    assert.deepEqual(result.rules, cases[index].rules, result.name),
  );
});

test("every workflow policy rule has a case or a repository example", () => {
  const seen = new Set(
    cases.flatMap((entry) => entry.rules.map((rule) => rule.split(":")[0])),
  );
  assert.deepEqual(
    RULES.filter(
      (rule) =>
        !seen.has(rule) && !["unlisted-file", "missing-file"].includes(rule),
    ),
    [],
  );
});

test("the repository workflows pass the policy", () => {
  assert.deepEqual(repositoryFindings(repoRoot, policy, toolSpec), []);
});

test("the policy pins the reviewed actions and grants issue writes only to the notify jobs", () => {
  assert.deepEqual(policy.permissions, { contents: "read" });
  assert.deepEqual(Object.keys(policy.jobPermissions).sort(), [
    ".github/workflows/ci.yml#notify",
    ".github/workflows/nightly.yml#notify",
  ]);
  Object.values(policy.jobPermissions).forEach((permissions) =>
    assert.deepEqual(permissions, { issues: "write" }),
  );
  assert.ok(!policy.events.includes("pull_request_target"));
  assert.ok(!policy.events.includes("workflow_run"));
  Object.values(policy.actions).forEach((sha) =>
    assert.match(sha, /^[0-9a-f]{40}$/u),
  );
});

const variants = (value, path = []) => {
  if (Array.isArray(value)) {
    return value.flatMap((item, index) => [
      {
        name: `${path.join(".")}[${index}]`,
        replace: value.filter((_, other) => other !== index),
        path,
      },
      ...variants(item, [...path, index]),
    ]);
  }
  if (typeof value === "object" && value !== null) {
    return Object.entries(value).flatMap(([key, item]) => [
      {
        name: [...path, key].join("."),
        replace: Object.fromEntries(
          Object.entries(value).filter(([other]) => other !== key),
        ),
        path,
      },
      ...variants(item, [...path, key]),
    ]);
  }
  return [];
};

const withReplaced = (document, path, replacement) => {
  if (path.length === 0) {
    return replacement;
  }
  const [head, ...rest] = path;
  const copy = Array.isArray(document) ? [...document] : { ...document };
  copy[head] = withReplaced(document[head], rest, replacement);
  return copy;
};

test("removing any element of the workflow policy breaks the repository or a case", () => {
  const surviving = variants(policy)
    .map((variant) => ({
      name: variant.name,
      candidate: withReplaced(policy, variant.path, variant.replace),
    }))
    .filter(({ candidate }) => {
      try {
        return (
          repositoryFindings(repoRoot, candidate, toolSpec).length === 0 &&
          matchesCases(candidate)
        );
      } catch {
        return false;
      }
    })
    .map(({ name }) => name);
  assert.deepEqual(surviving, []);
});

test("an unlisted file under .github and a missing workflow fail the repository check", () => {
  const directory = scratch("workflow-policy-");
  try {
    cpSync(join(repoRoot, ".github"), join(directory, ".github"), {
      recursive: true,
    });
    mkdirSync(join(directory, "harness"), { recursive: true });
    writeFileSync(join(directory, ".github", "dependabot.yml"), "version: 2\n");
    rmSync(join(directory, ".github", "workflows", "nightly.yml"));
    assert.deepEqual(repositoryFindings(directory, policy, toolSpec), [
      { path: ".github/dependabot.yml", rule: "unlisted-file" },
      { path: ".github/workflows/nightly.yml", rule: "missing-file" },
    ]);
    const finished = spawnSync(
      "node",
      ["harness/ci/workflow-policy/check.mjs", directory],
      { cwd: repoRoot, encoding: "utf8" },
    );
    assert.equal(finished.status, 1);
    assert.match(
      finished.stdout,
      /FAIL \.github\/dependabot\.yml: workflow:unlisted-file/u,
    );
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("the policy file is the one the check reads", () => {
  assert.equal(POLICY_FILE, "harness/ci/workflow-policy/policy.json");
});
