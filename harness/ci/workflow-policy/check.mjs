import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { installActionSpec, readTools } from "../tools.mjs";

export const POLICY_FILE = "harness/ci/workflow-policy/policy.json";

export const GITHUB_DIRECTORY = ".github";

export const RULES = [
  "not-canonical-json",
  "event",
  "permissions",
  "job-permissions",
  "runner",
  "continue-on-error",
  "reusable-workflow",
  "unpinned-action",
  "unlisted-action",
  "checkout-credentials",
  "tool-versions",
  "expression-in-run",
  "secrets",
  "unlisted-file",
  "missing-file",
];

const PINNED_ACTION =
  /^([A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+)(?:\/[^@]+)?@([0-9a-f]{40})$/u;

export const readPolicy = (root) =>
  JSON.parse(readFileSync(join(root, POLICY_FILE), "utf8"));

const isObject = (value) =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const eventNames = (on) =>
  typeof on === "string"
    ? [on]
    : Array.isArray(on)
      ? on.map(String)
      : isObject(on)
        ? Object.keys(on)
        : [String(on)];

const finding = (rule, detail) =>
  detail === undefined ? rule : `${rule}:${detail}`;

const stepFindings = (job, step, policy, toolSpec) => {
  if (!isObject(step)) {
    return [];
  }
  const found = [];
  if ("continue-on-error" in step) {
    found.push(finding("continue-on-error", job));
  }
  if (typeof step.run === "string" && step.run.includes("${{")) {
    found.push(finding("expression-in-run", job));
  }
  if (step.uses !== undefined) {
    const pinned = PINNED_ACTION.exec(String(step.uses));
    if (pinned === null) {
      found.push(finding("unpinned-action", step.uses));
    } else if (policy.actions[pinned[1]] !== pinned[2]) {
      found.push(finding("unlisted-action", step.uses));
    } else {
      if (
        pinned[1] === policy.checkout &&
        !(isObject(step.with) && step.with["persist-credentials"] === false)
      ) {
        found.push(finding("checkout-credentials", job));
      }
      if (
        pinned[1] === policy.toolInstaller &&
        !(isObject(step.with) && step.with.tool === toolSpec)
      ) {
        found.push(finding("tool-versions", job));
      }
    }
  }
  return found;
};

const jobFindings = (path, name, job, policy, toolSpec) => {
  if (!isObject(job)) {
    return [];
  }
  const expected = policy.jobPermissions[`${path}#${name}`];
  return [
    ...(isDeepStrictEqual(job.permissions, expected)
      ? []
      : [finding("job-permissions", name)]),
    ...(policy.runners.includes(job["runs-on"])
      ? []
      : [finding("runner", name)]),
    ...("continue-on-error" in job ? [finding("continue-on-error", name)] : []),
    ...(job.uses === undefined ? [] : [finding("reusable-workflow", name)]),
    ...(Array.isArray(job.steps) ? job.steps : []).flatMap((step) =>
      stepFindings(name, step, policy, toolSpec),
    ),
  ];
};

export const workflowFindings = (path, text, policy, toolSpec) => {
  const parsed = (() => {
    try {
      return JSON.parse(text);
    } catch {
      return undefined;
    }
  })();
  if (
    !isObject(parsed) ||
    text !== `${JSON.stringify(parsed, undefined, 2)}\n`
  ) {
    return [finding("not-canonical-json")];
  }
  const found = [
    ...eventNames(parsed.on)
      .filter((event) => !policy.events.includes(event))
      .map((event) => finding("event", event)),
    ...(isDeepStrictEqual(parsed.permissions, policy.permissions)
      ? []
      : [finding("permissions")]),
    ...Object.entries(isObject(parsed.jobs) ? parsed.jobs : {}).flatMap(
      ([name, job]) => jobFindings(path, name, job, policy, toolSpec),
    ),
    ...(text.includes("secrets.") ? [finding("secrets")] : []),
  ];
  return [...new Set(found)].sort();
};

const filesUnder = (root, relative) =>
  readdirSync(join(root, relative), { withFileTypes: true }).flatMap(
    (entry) => {
      const path = `${relative}/${entry.name}`;
      return entry.isDirectory() ? filesUnder(root, path) : [path];
    },
  );

export const repositoryFindings = (root, policy, toolSpec) => {
  const present = existsSync(join(root, GITHUB_DIRECTORY))
    ? filesUnder(root, GITHUB_DIRECTORY).sort()
    : [];
  return [
    ...present
      .filter((path) => !policy.files.includes(path))
      .map((path) => ({ path, rule: finding("unlisted-file") })),
    ...policy.files
      .filter((path) => !present.includes(path))
      .map((path) => ({ path, rule: finding("missing-file") })),
    ...policy.files
      .filter((path) => present.includes(path))
      .flatMap((path) =>
        workflowFindings(
          path,
          readFileSync(join(root, path), "utf8"),
          policy,
          toolSpec,
        ).map((rule) => ({ path, rule })),
      ),
  ];
};

const repoRoot = resolve(
  fileURLToPath(import.meta.url),
  "..",
  "..",
  "..",
  "..",
);

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = resolve(process.argv[2] ?? repoRoot);
  const found = repositoryFindings(
    root,
    readPolicy(repoRoot),
    installActionSpec(readTools(repoRoot)),
  );
  found.forEach((item) =>
    process.stdout.write(`FAIL ${item.path}: workflow:${item.rule}\n`),
  );
  process.stdout.write(`workflow-policy: ${found.length} failure(s)\n`);
  process.exitCode = found.length === 0 ? 0 : 1;
}
