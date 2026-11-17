import { spawnSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { gitEnvironment } from "./project.mjs";
import { globSource, hasGlob } from "./sandbox-glob.mjs";

export const SANDBOX_EXEC = "/usr/bin/sandbox-exec";

export const SENSITIVE_HOME = [
  ".ssh",
  ".aws",
  ".gnupg",
  ".config/gh",
  ".netrc",
  ".npmrc",
  ".docker",
  ".kube",
  "Library/Keychains",
  "Library/Cookies",
  "Library/Application Support/Google/Chrome",
  "Library/Application Support/BraveSoftware",
  "Library/Application Support/Microsoft Edge",
  "Library/Application Support/Firefox",
  "Library/Application Support/Arc",
  "Library/Safari",
  ".claude",
];

const SECRET_VARIABLE =
  /(TOKEN|SECRET|PASSWORD|PASSWD|CREDENTIAL|API_KEY|ACCESS_KEY|PRIVATE_KEY|AUTH)/iu;

const quoted = (path) => JSON.stringify(path);

const realOrSelf = (path) => {
  try {
    return realpathSync.native(path);
  } catch {
    return path;
  }
};

const below = (root, entry) => {
  const relative = entry.startsWith("./") ? entry.slice(2) : entry;
  return hasGlob(relative)
    ? `(regex #"^${root}/${globSource(relative)}(/.*)?$")`
    : `(subpath ${quoted(join(root, relative))})`;
};

export const checkPlaces = ({
  worktree,
  common,
  projectRoot,
  state,
  locked = [],
  home = homedir(),
  temporary = realOrSelf(tmpdir()),
  configDirectory = process.env.CLAUDE_CONFIG_DIR,
}) => {
  const tree = realOrSelf(worktree);
  const project = realOrSelf(projectRoot);
  return {
    temporary,
    writable: [
      `(subpath ${quoted(tree)})`,
      `(subpath ${quoted(temporary)})`,
      '(subpath "/dev")',
      '(regex #"^/private/tmp/pnpm-[a-z-]+-[0-9]+(/|$)")',
      `(subpath ${quoted(join(home, ".cargo/registry"))})`,
      `(literal ${quoted(join(home, ".cargo/advisory-dbs/db.lock"))})`,
      `(subpath ${quoted(join(home, "Library/pnpm/store"))})`,
      `(regex #${quoted(`^${join(home, "Library/pnpm")}/_tmp_[0-9]+_[0-9a-f]+(/[^/]+)?$`)})`,
    ],
    denied: [
      `(subpath ${quoted(realOrSelf(common))})`,
      `(subpath ${quoted(join(project, ".claude"))})`,
      `(subpath ${quoted(join(home, ".cargo/registry/src"))})`,
      `(subpath ${quoted(state)})`,
      ...[...new Set([tree, project])].flatMap((root) =>
        locked.map((entry) => below(root, entry)),
      ),
    ],
    unreadable: [
      ...SENSITIVE_HOME.map((path) => `(subpath ${quoted(join(home, path))})`),
      ...(typeof configDirectory === "string" && configDirectory.startsWith("/")
        ? [`(subpath ${quoted(realOrSelf(configDirectory))})`]
        : []),
      `(subpath ${quoted(state)})`,
    ],
  };
};

export const NETWORK_RULES =
  '(deny network-outbound)(allow network-outbound (remote ip "localhost:*"))';

export const checkProfile = (places) =>
  [
    "(version 1)(allow default)",
    NETWORK_RULES,
    `(allow network-outbound (remote unix-socket (path-regex #${quoted(`^${places.temporary}/`)})))`,
    "(deny file-write*)",
    `(allow file-write* ${places.writable.join(" ")})`,
    `(deny file-write* ${places.denied.join(" ")})`,
    `(deny file-read* ${places.unreadable.join(" ")})`,
  ].join("");

const STEP_TIMEOUT_MS = 1700000;

export const checkEnvironment = (
  temporary,
  environment = gitEnvironment(),
) => ({
  ...Object.fromEntries(
    Object.entries(environment).filter(
      ([name]) => name !== "SSH_AUTH_SOCK" && !SECRET_VARIABLE.test(name),
    ),
  ),
  TMPDIR: `${temporary}/`,
  SANDBOX_RUNTIME: "1",
  CARGO_NET_OFFLINE: "true",
  npm_config_offline: "true",
  npm_config_ignore_scripts: "true",
});

export const runInSandbox = (places, worktree, command) => {
  const finished = spawnSync(
    SANDBOX_EXEC,
    ["-p", checkProfile(places), ...command],
    {
      cwd: worktree,
      encoding: "utf8",
      env: checkEnvironment(places.temporary),
      stdio: ["ignore", "pipe", "pipe"],
      timeout: STEP_TIMEOUT_MS,
      maxBuffer: 256 * 1024 * 1024,
    },
  );
  return {
    status: finished.status ?? 1,
    output: `${finished.stdout ?? ""}${finished.stderr ?? ""}${finished.error === undefined ? "" : `\n${finished.error.message}`}`,
  };
};

export const FRESH_STEPS = [
  ["cargo", "clean", "--locked", "--offline", "--workspace"],
  ["cargo", "llvm-cov", "clean", "--locked", "--offline", "--workspace"],
  [
    "pnpm",
    "install",
    "--frozen-lockfile",
    "--force",
    "--offline",
    "--ignore-scripts",
  ],
];

const OFFLINE_HINT =
  "\n훅의 검사는 네트워크 없이 이미 받아 둔 의존성만 쓴다. 네트워크가 되는 곳에서 pnpm install --frozen-lockfile, cargo fetch --locked로 받아 둔 뒤 다시 끝내라.";

const runSteps = (places, worktree, steps, output) => {
  if (steps.length === 0) {
    return { status: 0, output };
  }
  const [step, ...rest] = steps;
  const ran = runInSandbox(places, worktree, step);
  const combined = `${output}$ ${step.join(" ")}\n${ran.output}`;
  return ran.status === 0
    ? runSteps(places, worktree, rest, combined)
    : { status: ran.status, output: `${combined}${OFFLINE_HINT}` };
};

export const runHarnessCheck = (
  places,
  worktree,
  base,
  steps = FRESH_STEPS,
) => {
  const fresh = runSteps(places, worktree, steps, "");
  if (fresh.status !== 0) {
    return fresh;
  }
  const check = runInSandbox(places, worktree, [
    "just",
    `base=${base}`,
    "check",
  ]);
  return { status: check.status, output: `${fresh.output}${check.output}` };
};
