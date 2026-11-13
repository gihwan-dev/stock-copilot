import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseAllowlist } from "../../../.claude/hooks/lib/allowlist.mjs";
import {
  ALLOWLIST_FILE,
  parseLock,
  verify,
  xtaskRunner,
} from "../signatures/verify.mjs";

export const repoRoot = resolve(
  fileURLToPath(import.meta.url),
  "..",
  "..",
  "..",
  "..",
);

export const EXAMPLE = "harness/test-diff/repository";

export const MEMBER_MANIFEST = "crates/sample/Cargo.toml";

export const EXTRA_TESTS = "crates/sample/src/extra_tests.rs";

export const LIBRARY = "crates/sample/src/lib.rs";

export const SCRIPT_TEST = "crates/sample/src/probe.test.mjs";

export const PACKAGE = "apps/desktop/package.json";

export const PROTECTED = "justfile";

const PACKAGE_TEXT = `${JSON.stringify(
  {
    name: "desktop",
    private: true,
    scripts: { build: "vite build" },
    dependencies: { react: "19.2.0" },
  },
  undefined,
  2,
)}\n`;

export const CHILD_ENVIRONMENT = [
  "PATH",
  "HOME",
  "TMPDIR",
  "LANG",
  "LC_ALL",
  "USER",
  "CARGO_HOME",
  "RUSTUP_HOME",
];

export const childEnvironment = (extra = {}) => ({
  ...Object.fromEntries(
    Object.entries(process.env).filter(([name]) =>
      CHILD_ENVIRONMENT.includes(name),
    ),
  ),
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_CONFIG_GLOBAL: "/dev/null",
  ...extra,
});

const environment = () => childEnvironment();

export const workflowSteps = (name, job) =>
  JSON.parse(readFileSync(join(repoRoot, ".github", "workflows", name), "utf8"))
    .jobs[job].steps;

export const SHELL_FLAGS = ["--noprofile", "--norc", "-c"];

export const runBaseStep = (made, event, runs) => {
  const temporary = scratch("verified-base-");
  try {
    const bin = join(temporary, "bin");
    write(temporary, "bin/gh", '#!/bin/sh\ncat "$FAKE_RUNS"\n');
    chmodSync(join(bin, "gh"), 0o755);
    write(temporary, "runs.txt", runs.map((sha) => `${sha}\n`).join(""));
    write(temporary, "output.txt", "");
    const step = workflowSteps("signatures.yml", "signatures").find(
      (candidate) => candidate.id === "base",
    );
    const finished = spawnSync("bash", [...SHELL_FLAGS, step.run], {
      cwd: made.directory,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
      env: childEnvironment({
        PATH: `${bin}:${process.env.PATH ?? ""}`,
        HOME: temporary,
        FAKE_RUNS: join(temporary, "runs.txt"),
        RUNNER_TEMP: temporary,
        GITHUB_OUTPUT: join(temporary, "output.txt"),
        GITHUB_REPOSITORY: "owner/repository",
        ...event,
      }),
    });
    return {
      status: finished.status,
      output: Object.fromEntries(
        readFileSync(join(temporary, "output.txt"), "utf8")
          .split("\n")
          .filter((line) => line !== "")
          .map((line) => line.split("=")),
      ),
    };
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
};

export const workflowRun = (made, step, extra) =>
  spawnSync("bash", [...SHELL_FLAGS, step.run], {
    cwd: made.directory,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    env: childEnvironment({ GITHUB_WORKSPACE: made.directory, ...extra }),
  });

export const scratch = (prefix) =>
  mkdtempSync(join(realpathSync(tmpdir()), prefix));

export const run = (directory, command, args) => {
  const finished = spawnSync(command, args, {
    cwd: directory,
    encoding: "utf8",
    env: environment(),
  });
  assert.equal(
    finished.status,
    0,
    `${command} ${args.join(" ")}: ${finished.stderr}`,
  );
  return finished.stdout.trim();
};

const IDENTITY = [
  "-c",
  "user.name=harness",
  "-c",
  "user.email=harness@example.invalid",
  "-c",
  "commit.gpgsign=false",
  "-c",
  "gpg.format=ssh",
];

export const gitIn = (directory, args) =>
  run(directory, "git", [...IDENTITY, ...args]);

export const makeKey = (directory, name) => {
  const file = join(directory, name);
  run(directory, "ssh-keygen", [
    "-q",
    "-t",
    "ed25519",
    "-N",
    "",
    "-C",
    name,
    "-f",
    file,
  ]);
  return { file, public: readFileSync(`${file}.pub`, "utf8").trim() };
};

export const signersFor = (...keys) =>
  keys
    .map((key) => `harness@example.invalid namespaces="git" ${key.public}\n`)
    .join("");

export const write = (directory, path, text) => {
  mkdirSync(dirname(join(directory, path)), { recursive: true });
  writeFileSync(join(directory, path), text);
};

export const read = (directory, path) =>
  readFileSync(join(directory, path), "utf8");

export const edit = (directory, path, from, to) => {
  const text = read(directory, path);
  assert.ok(text.includes(from), `${path} lacks ${from}`);
  write(directory, path, text.replace(from, to));
};

export const commit = (directory, message, key) => {
  gitIn(directory, ["add", "--all"]);
  gitIn(directory, [
    ...(key === undefined ? [] : ["-c", `user.signingkey=${key.file}`]),
    "commit",
    "--quiet",
    "--allow-empty",
    ...(key === undefined ? [] : ["-S"]),
    "-m",
    message,
  ]);
  return gitIn(directory, ["rev-parse", "HEAD"]);
};

export const createRepository = () => {
  const directory = scratch("signature-repo-");
  cpSync(join(repoRoot, EXAMPLE), directory, { recursive: true });
  write(directory, PROTECTED, "check:\n    true\n");
  write(directory, PACKAGE, PACKAGE_TEXT);
  write(directory, SCRIPT_TEST, 'test("probe", () => {});\n');
  write(directory, "harness/ci/signatures/lock-point.txt", "");
  gitIn(directory, ["init", "--quiet", "--initial-branch=master"]);
  const base = commit(directory, "base");
  return { directory, base };
};

export const removeRepository = (directory) =>
  rmSync(directory, { recursive: true, force: true });

export const realAllowlist = () =>
  parseAllowlist(readFileSync(join(repoRoot, ALLOWLIST_FILE), "utf8"));

export const ownerKeyOf = (key) => key.public.split(" ").slice(0, 2).join(" ");

export const verifyIn = ({
  directory,
  base,
  head,
  lock,
  signers,
  ownerKeys,
  baseKind,
}) =>
  verify({
    repo: directory,
    base,
    head,
    trust: {
      lock: parseLock(lock === undefined ? "" : `${lock}\n`),
      signers,
      allowlist: realAllowlist(),
      configNames: JSON.parse(
        readFileSync(join(repoRoot, "harness/claude-hooks/rules.json"), "utf8"),
      ).paths.configFiles.names,
    },
    xtask: xtaskRunner(repoRoot),
    ownerKeys:
      ownerKeys === undefined ? undefined : () => ({ keys: ownerKeys }),
    baseKind: baseKind ?? "verified",
  });
