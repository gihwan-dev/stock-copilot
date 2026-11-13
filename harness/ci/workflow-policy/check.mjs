import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual } from "node:util";
import { installActionSpec, readTools } from "../tools.mjs";

export const POLICY_FILE = "harness/ci/workflow-policy/policy.json";

export const GITHUB_DIRECTORY = ".github";

export const RULES = [
  "not-canonical-json",
  "unknown-key",
  "missing-key",
  "event",
  "permissions",
  "job-permissions",
  "runner",
  "timeout",
  "needs",
  "condition",
  "step-shape",
  "command",
  "expression",
  "secrets",
  "unlisted-action",
  "action-input",
  "environment",
  "checkout-credentials",
  "tool-versions",
  "expected",
  "unlisted-file",
  "missing-file",
];

const PINNED_ACTION = /^([^@\s]+)@([0-9a-f]{40})$/u;

const SECRETS = /\bsecrets\b/u;

export const readPolicy = (root) =>
  JSON.parse(readFileSync(join(root, POLICY_FILE), "utf8"));

const isObject = (value) =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const finding = (rule, detail) =>
  detail === undefined ? rule : `${rule}:${detail}`;

const keyFindings = (value, allowed, required, where) =>
  isObject(value)
    ? [
        ...Object.keys(value)
          .filter((key) => !allowed.includes(key))
          .map((key) => finding("unknown-key", `${where}.${key}`)),
        ...required
          .filter((key) => !(key in value))
          .map((key) => finding("missing-key", `${where}.${key}`)),
      ]
    : [finding("missing-key", where)];

const valueFindings = (values, policy, where) =>
  Object.values(isObject(values) ? values : {}).flatMap((value) =>
    typeof value === "string" &&
    (!value.includes("${{") || policy.expressions.includes(value))
      ? []
      : typeof value === "number" || typeof value === "boolean"
        ? []
        : [finding("expression", where)],
  );

const eventFindings = (on, policy) => {
  if (!isObject(on)) {
    return [finding("event", String(on))];
  }
  return Object.entries(on).flatMap(([event, options]) => {
    const allowed = policy.events[event];
    if (allowed === undefined) {
      return [finding("event", event)];
    }
    if (event === "schedule") {
      return Array.isArray(options) &&
        options.every(
          (entry) =>
            isObject(entry) &&
            Object.keys(entry).length === 1 &&
            policy.schedules.includes(entry.cron),
        )
        ? []
        : [finding("event", "schedule")];
    }
    return keyFindings(options, allowed, [], `on.${event}`);
  });
};

const stepFindings = (job, step, policy, toolSpec) => {
  const where = `jobs.${job}.steps`;
  if (!isObject(step)) {
    return [finding("step-shape", job)];
  }
  const found = [
    ...keyFindings(step, policy.stepKeys, ["name"], where),
    ...("uses" in step === "run" in step ? [finding("step-shape", job)] : []),
    ...(step.if === undefined || policy.conditions.includes(step.if)
      ? []
      : [finding("condition", job)]),
    ...valueFindings(step.env, policy, job),
    ...Object.keys(isObject(step.env) ? step.env : {})
      .filter((name) => !policy.envNames.includes(name))
      .map(() => finding("environment", job)),
  ];
  if ("run" in step && !policy.commands.includes(step.run)) {
    found.push(finding("command", job));
  }
  if ("uses" in step) {
    const pinned = PINNED_ACTION.exec(String(step.uses));
    const action = pinned === null ? undefined : policy.actions[pinned[1]];
    if (action === undefined || action.sha !== pinned[2]) {
      found.push(finding("unlisted-action", step.uses));
    } else {
      const inputs = isObject(step.with) ? step.with : {};
      if (
        Object.entries(inputs).some(
          ([key, value]) =>
            !action.inputs.includes(key) ||
            (key !== "tool" &&
              !(policy.inputValues[key] ?? []).some((allowed) =>
                isDeepStrictEqual(allowed, value),
              )),
        )
      ) {
        found.push(finding("action-input", job));
      }
      if (
        pinned[1] === policy.checkout &&
        inputs["persist-credentials"] !== false
      ) {
        found.push(finding("checkout-credentials", job));
      }
      if (pinned[1] === policy.toolInstaller && inputs.tool !== toolSpec) {
        found.push(finding("tool-versions", job));
      }
    }
  }
  return found;
};

const jobFindings = (path, name, job, jobs, policy, toolSpec) => {
  if (!isObject(job)) {
    return [finding("missing-key", `jobs.${name}`)];
  }
  const expected = policy.jobPermissions[`${path}#${name}`];
  const timeout = job["timeout-minutes"];
  return [
    ...keyFindings(job, policy.jobKeys, policy.requiredJobKeys, `jobs.${name}`),
    ...(isDeepStrictEqual(job.permissions, expected)
      ? []
      : [finding("job-permissions", name)]),
    ...(policy.runners.includes(job["runs-on"])
      ? []
      : [finding("runner", name)]),
    ...(Number.isInteger(timeout) && timeout > 0 && timeout <= policy.maxTimeout
      ? []
      : [finding("timeout", name)]),
    ...(job.needs === undefined ||
    (Array.isArray(job.needs) &&
      job.needs.every((other) => other !== name && other in jobs))
      ? []
      : [finding("needs", name)]),
    ...(job.if === undefined || policy.conditions.includes(job.if)
      ? []
      : [finding("condition", name)]),
    ...valueFindings(job.outputs, policy, name),
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
  const jobs = isObject(parsed.jobs) ? parsed.jobs : {};
  const found = [
    ...keyFindings(parsed, policy.topKeys, policy.topKeys, "top"),
    ...eventFindings(parsed.on, policy),
    ...(isDeepStrictEqual(parsed.permissions, policy.permissions)
      ? []
      : [finding("permissions")]),
    ...Object.entries(jobs).flatMap(([name, job]) =>
      jobFindings(path, name, job, jobs, policy, toolSpec),
    ),
    ...(SECRETS.test(text) ? [finding("secrets")] : []),
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
  const files = Object.keys(policy.expected).sort();
  const present = existsSync(join(root, GITHUB_DIRECTORY))
    ? filesUnder(root, GITHUB_DIRECTORY).sort()
    : [];
  return [
    ...present
      .filter((path) => !files.includes(path))
      .map((path) => ({ path, rule: finding("unlisted-file") })),
    ...files
      .filter((path) => !present.includes(path))
      .map((path) => ({ path, rule: finding("missing-file") })),
    ...files
      .filter((path) => present.includes(path))
      .flatMap((path) => {
        const text = readFileSync(join(root, path), "utf8");
        const expected = `${JSON.stringify(policy.expected[path], undefined, 2)}\n`;
        return [
          ...workflowFindings(path, text, policy, toolSpec),
          ...workflowFindings(path, expected, policy, toolSpec).map(
            (rule) => `policy-expected:${rule}`,
          ),
          ...(text === expected ? [] : [finding("expected")]),
        ].map((rule) => ({ path, rule }));
      }),
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
