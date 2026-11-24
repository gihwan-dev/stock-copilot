import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const HOOKS_FILE = ".agents/hooks.json";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");

const shellQuote = (text) => `'${text.replaceAll("'", "'\\''")}'`;

export const eventHookCommand = (event) => {
  const reason = "Antigravity 하네스 시작 실패: Git 또는 Node를 확인하라";
  const output =
    event === "PostToolUse"
      ? {}
      : event === "Stop"
        ? { decision: "continue", reason }
        : { decision: "deny", reason };
  const body =
    "if common=$(/usr/bin/env -i GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 /usr/bin/git rev-parse --path-format=absolute --git-common-dir); then " +
    'main_root="${common%/*}"; ' +
    'target="$main_root/harness/antigravity-hooks/dispatch.mjs"; ' +
    'if [ ! -f "$target" ] && ! /usr/bin/env -i GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 /usr/bin/git -C "$main_root" cat-file -e HEAD:harness/antigravity-hooks/dispatch.mjs 2>/dev/null && [ -f "../harness/antigravity-hooks/dispatch.mjs" ]; then target="../harness/antigravity-hooks/dispatch.mjs"; fi; ' +
    'if [ -f "$target" ]; then if result=$(node "$target" ' +
    shellQuote(event) +
    '); then if [ -n "$result" ]; then printf \'%s\\n\' "$result"; exit 0; fi; fi; fi; fi; ' +
    "printf '%s\\n' " +
    shellQuote(JSON.stringify(output)) +
    "; printf '%s\\n' " +
    shellQuote(reason) +
    " >&2; exit 0";
  return `/bin/sh -c ${shellQuote(body)}`;
};

export const hooksConfig = () => ({
  "stock-copilot-harness": {
    PreToolUse: [
      {
        matcher: "*",
        hooks: [
          {
            type: "command",
            command: eventHookCommand("PreToolUse"),
            timeout: 30,
          },
        ],
      },
    ],
    PostToolUse: [
      {
        matcher: "*",
        hooks: [
          {
            type: "command",
            command: eventHookCommand("PostToolUse"),
            timeout: 30,
          },
        ],
      },
    ],
    Stop: [
      {
        type: "command",
        command: eventHookCommand("Stop"),
        timeout: 120,
      },
    ],
  },
});

export const hooksText = () =>
  `${JSON.stringify(hooksConfig(), undefined, 2)}\n`;

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const target = join(root, HOOKS_FILE);
  const expected = hooksText();
  if (process.argv[2] === "--write") {
    mkdirSync(dirname(target), { recursive: true });
    writeFileSync(target, expected);
  }
  let actual;
  try {
    actual = readFileSync(target, "utf8");
  } catch {
    actual = undefined;
  }
  if (actual !== expected) {
    process.stderr.write(
      `${HOOKS_FILE}: node harness/antigravity-hooks/config.mjs --write로 다시 생성하라\n`,
    );
    process.exitCode = 1;
  }
}
