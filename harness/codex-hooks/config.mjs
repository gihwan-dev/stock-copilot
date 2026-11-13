import { readFileSync, writeFileSync, realpathSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  readPolicy,
  readAllowlistFile,
  protectedDenies,
  trackedProtectedDenies,
} from "../sandbox/fragment.mjs";
import { git, readLock } from "../../.claude/hooks/lib/project.mjs";
const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const entry = (path, mode) =>
  JSON.stringify(path) + " = " + JSON.stringify(mode);
const tracked = git(root, ["ls-files", "-z"])?.split("\0").filter(Boolean);
if (tracked === undefined)
  throw new Error("보호 경로의 Git 목록을 읽지 못했다");
const policy = readPolicy(root);
export const PROTECTED = [
  ...new Set([
    ".claude",
    ".codex",
    ".cargo",
    "harness",
    "justfile",
    "crates/xtask",
    "lefthook.yml",
    ".github",
    "AGENTS.md",
    "CLAUDE.md",
    "docs/agents",
    "docs/harness.md",
    "docs/workflow.md",
    "docs/open-issues.md",
    "docs/progress.md",
    ...protectedDenies(
      tracked,
      readAllowlistFile(root),
      policy.lock.keepWritable,
    ),
    ...trackedProtectedDenies(tracked, policy).map((path) => path.slice(2)),
  ]),
];
export const configText = (policy) => {
  const fs = policy.base.filesystem;
  return [
    'default_permissions = "stock-copilot"',
    'approval_policy = "never"',
    "allow_login_shell = false",
    "[features]",
    "hooks = true",
    "shell_snapshot = false",
    "network_proxy = true",
    "[shell_environment_policy]",
    'inherit = "core"',
    "ignore_default_excludes = false",
    "[permissions.stock-copilot]",
    'extends = ":workspace"',
    "[permissions.stock-copilot.filesystem]",
    ...fs.allowWrite
      .filter((path) => path.startsWith("~/") && !path.includes("*"))
      .map((path) => entry(path, "write")),
    ...fs.denyWrite
      .filter(
        (path) =>
          path.startsWith("~/") &&
          !path.includes("*") &&
          !fs.denyRead.includes(path),
      )
      .map((path) => entry(path, "read")),
    ...fs.denyRead
      .filter((path) => !path.includes("*"))
      .map((path) => entry(path, "deny")),
    entry("~/.codex", "deny"),
    '[permissions.stock-copilot.filesystem.":workspace_roots"]',
    ...PROTECTED.map((path) => entry(path, "read")),
    "[permissions.stock-copilot.network]",
    "enabled = true",
    "allow_local_binding = " +
      JSON.stringify(policy.base.network.allowLocalBinding),
    "[permissions.stock-copilot.network.domains]",
    ...policy.base.network.allowedDomains.map((domain) =>
      entry(domain, "allow"),
    ),
    "",
  ].join("\n");
};
export const hookCommand =
  "/bin/sh -c 'common=$(/usr/bin/env -i GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 /usr/bin/git rev-parse --path-format=absolute --git-common-dir) || exit 1; exec node \"${common%/*}/harness/codex-hooks/dispatch.mjs\"'";
export const hooksConfig = () => ({
  hooks: Object.fromEntries(
    [
      ["PreToolUse", 600],
      ["PostToolUse", 300],
      ["Stop", 3600],
    ].map(([event, timeout]) => [
      event,
      [{ hooks: [{ type: "command", command: hookCommand, timeout }] }],
    ]),
  ),
});
export const setupText = (
  cwd,
  home = homedir(),
  nodeDirectory = dirname(process.execPath),
  harness = "locked",
  temporary = realpathSync(tmpdir()),
) => {
  const common = git(cwd, [
    "rev-parse",
    "--path-format=absolute",
    "--git-common-dir",
  ])?.trim();
  const gitdir = git(cwd, ["rev-parse", "--absolute-git-dir"])?.trim();
  const branch = git(cwd, ["symbolic-ref", "--short", "HEAD"])?.trim();
  if (
    !common ||
    !gitdir ||
    gitdir === common ||
    !branch ||
    ["master", "main"].includes(branch)
  )
    throw new Error("설정 출력은 등록된 구현 worktree에서 실행한다");
  if (harness === "harness" && readLock(dirname(common)) !== "off")
    throw new Error("잠금 on에서는 하네스 쓰기를 열지 않는다");
  const paths = [
    home + "/Library/pnpm",
    nodeDirectory,
    home + "/.cargo/bin",
    "/opt/homebrew/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
  ];
  const writable = [
    gitdir,
    common + "/objects",
    common + "/refs/heads/" + branch,
    common + "/refs/heads/" + branch + ".lock",
    common + "/logs/refs/heads/" + branch,
  ];
  const profile =
    harness === "harness" ? "stock-copilot-harness" : "stock-copilot-session";
  return [
    "[shell_environment_policy.set]",
    entry("PATH", [...new Set(paths)].join(":")),
    entry("pnpm_config_store_dir", home + "/Library/pnpm/store"),
    "[permissions." + profile + "]",
    'extends = "stock-copilot"',
    "[permissions." + profile + ".filesystem]",
    ...writable.map((path) => entry(path, "write")),
    ...["commondir", "gitdir", "config.worktree"].map((name) =>
      entry(gitdir + "/" + name, "read"),
    ),
    entry(common + "/hooks", "read"),
    entry(common + "/config", "read"),
    "[permissions." + profile + ".network.unix_sockets]",
    entry(temporary, "allow"),
    "[permissions." + profile + '.filesystem.":workspace_roots"]',
    ...(harness === "harness"
      ? PROTECTED.filter(
          (path) => path !== ".cargo" && !path.startsWith(".cargo/"),
        ).map((path) => entry(path, "write"))
      : []),
    "",
  ].join("\n");
};
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  if (process.argv.includes("--setup"))
    process.stdout.write(
      setupText(
        process.cwd(),
        homedir(),
        dirname(process.execPath),
        process.argv.includes("--harness") ? "harness" : "locked",
      ),
    );
  else
    for (const [path, expected] of [
      [".codex/config.toml", configText(readPolicy(root))],
      [".codex/hooks.json", JSON.stringify(hooksConfig(), undefined, 2) + "\n"],
    ]) {
      if (process.argv[2] === "--write")
        writeFileSync(join(root, path), expected);
      if (readFileSync(join(root, path), "utf8") !== expected) {
        process.stderr.write(
          path +
            ": node harness/codex-hooks/config.mjs --write로 다시 생성하라\n",
        );
        process.exitCode = 1;
      }
    }
}
