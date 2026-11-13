import { readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { readPolicy } from "../sandbox/fragment.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const quote = JSON.stringify;
export const configText = (policy, home, nodeDirectory) => {
  const fs = policy.base.filesystem;
  const paths = [
    `${home}/Library/pnpm`,
    nodeDirectory,
    `${home}/.cargo/bin`,
    "/opt/homebrew/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
  ];
  const expand = (path) =>
    path.startsWith("~/") ? `${home}/${path.slice(2)}` : path;
  const writes = fs.allowWrite.filter(
    (path) => !path.startsWith("../") && !path.includes("*"),
  );
  const denies = fs.denyRead.filter((path) => !path.includes("*"));
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
    `[shell_environment_policy.set]`,
    `PATH = ${quote([...new Set(paths)].join(":"))}`,
    "[permissions.stock-copilot]",
    'extends = ":workspace"',
    "[permissions.stock-copilot.filesystem]",
    ...writes.map((path) => `${quote(expand(path))} = "write"`),
    ...fs.denyWrite
      .filter(
        (path) =>
          path.startsWith("~/") &&
          !path.includes("*") &&
          !denies.includes(path),
      )
      .map((path) => `${quote(expand(path))} = "read"`),
    ...denies.map((path) => `${quote(expand(path))} = "deny"`),
    `${quote(`${home}/.codex`)} = "deny"`,
    '[permissions.stock-copilot.filesystem.":workspace_roots"]',
    '".claude" = "read"',
    '".codex" = "read"',
    '".cargo" = "read"',
    ...fs.denyWrite
      .filter((path) => path.startsWith("./"))
      .map((path) => `${quote(path.slice(2))} = "read"`),
    "[permissions.stock-copilot.network]",
    "enabled = true",
    "[permissions.stock-copilot.network.domains]",
    ...policy.base.network.allowedDomains.map(
      (domain) => `${quote(domain)} = "allow"`,
    ),
    "[permissions.stock-copilot.network.unix_sockets]",
    ...policy.base.network.allowUnixSockets.map(
      (path) => `${quote(path)} = "allow"`,
    ),
    "",
  ].join("\n");
};

export const hooksConfig = () => ({
  hooks: Object.fromEntries(
    [
      ["PreToolUse", 600],
      ["PostToolUse", 300],
      ["Stop", 3600],
    ].map(([event, timeout]) => [
      event,
      [
        {
          hooks: [
            {
              type: "command",
              command:
                'node "$(git rev-parse --show-toplevel)/harness/codex-hooks/dispatch.mjs"',
              timeout,
            },
          ],
        },
      ],
    ]),
  ),
});

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const files = [
    [
      ".codex/config.toml",
      configText(readPolicy(root), homedir(), dirname(process.execPath)),
    ],
    [".codex/hooks.json", `${JSON.stringify(hooksConfig(), undefined, 2)}\n`],
  ];
  for (const [path, expected] of files) {
    if (process.argv[2] === "--write")
      writeFileSync(join(root, path), expected);
    if (readFileSync(join(root, path), "utf8") !== expected) {
      process.stderr.write(
        `${path}: node harness/codex-hooks/config.mjs --write로 현재 컴퓨터 설정을 생성하라\n`,
      );
      process.exitCode = 1;
    }
  }
}
