import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import {
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

const environment = () => ({
  ...Object.fromEntries(
    Object.entries(process.env).filter(([name]) => !name.startsWith("GIT_")),
  ),
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_CONFIG_GLOBAL: "/dev/null",
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
  gitIn(directory, ["init", "--quiet", "--initial-branch=master"]);
  const base = commit(directory, "base");
  return { directory, base };
};

export const removeRepository = (directory) =>
  rmSync(directory, { recursive: true, force: true });

export const realAllowlist = () =>
  parseAllowlist(readFileSync(join(repoRoot, ALLOWLIST_FILE), "utf8"));

export const verifyIn = ({ directory, base, head, lock, signers }) =>
  verify({
    repo: directory,
    base,
    head,
    trust: {
      lock: parseLock(lock === undefined ? "" : `${lock}\n`),
      signers,
      allowlist: realAllowlist(),
    },
    xtask: xtaskRunner(repoRoot),
  });
