import { spawnSync } from "node:child_process";
import { realpathSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { gitEnvironment } from "./project.mjs";

export const SANDBOX_EXEC = "/usr/bin/sandbox-exec";

export const NETWORK_RULES =
  '(deny network-outbound)(allow network-outbound (remote ip "localhost:*"))(allow network-outbound (remote unix-socket))(deny network-outbound (remote unix-socket (path-literal "/private/var/run/mDNSResponder")))';

const quoted = (path) => JSON.stringify(path);

const realOrSelf = (path) => {
  try {
    return realpathSync.native(path);
  } catch {
    return path;
  }
};

export const checkPlaces = ({
  worktree,
  common,
  projectRoot,
  state,
  home = homedir(),
  temporary = realOrSelf(tmpdir()),
}) => ({
  writable: [
    `(subpath ${quoted(realOrSelf(worktree))})`,
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
    `(subpath ${quoted(join(realOrSelf(projectRoot), ".claude"))})`,
    `(subpath ${quoted(join(home, ".cargo/registry/src"))})`,
    `(subpath ${quoted(state)})`,
  ],
});

export const checkProfile = (places, network) =>
  [
    "(version 1)(allow default)",
    network === "blocked" ? NETWORK_RULES : "",
    "(deny file-write*)",
    `(allow file-write* ${places.writable.join(" ")})`,
    `(deny file-write* ${places.denied.join(" ")})`,
  ].join("");

const STEP_TIMEOUT_MS = 1700000;

export const runInSandbox = (profile, worktree, command) => {
  const finished = spawnSync(SANDBOX_EXEC, ["-p", profile, ...command], {
    cwd: worktree,
    encoding: "utf8",
    env: { ...gitEnvironment(), SANDBOX_RUNTIME: "1" },
    stdio: ["ignore", "pipe", "pipe"],
    timeout: STEP_TIMEOUT_MS,
    maxBuffer: 256 * 1024 * 1024,
  });
  return {
    status: finished.status,
    output: `${finished.stdout ?? ""}${finished.stderr ?? ""}${finished.error === undefined ? "" : `\n${finished.error.message}`}`,
  };
};

export const runHarnessCheck = (places, worktree, base) => {
  const build = runInSandbox(checkProfile(places, "open"), worktree, [
    "just",
    "fresh-build",
  ]);
  if (build.status !== 0) {
    return { status: build.status ?? 1, output: build.output };
  }
  const check = runInSandbox(checkProfile(places, "blocked"), worktree, [
    "just",
    `base=${base}`,
    "check",
  ]);
  return {
    status: check.status ?? 1,
    output: `${build.output}${check.output}`,
  };
};
