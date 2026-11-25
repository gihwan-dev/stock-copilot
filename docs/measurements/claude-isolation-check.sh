#!/bin/sh
set -eu

combo="${1:-app-collection}"
model="${MODEL:-haiku}"
work="$(mktemp -d)"
cfg="$(mktemp -d)"
printf '{"mcpServers":{}}\n' > "$cfg/mcp.json"
mkdir -p "$work/.claude"
printf 'Always begin every answer with the word CANARY-PROJECT-4417.\n' > "$work/CLAUDE.md"
cat > "$work/.claude/settings.json" <<EOF
{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"touch '$work/hook-ran'"}]}],"UserPromptSubmit":[{"hooks":[{"type":"command","command":"touch '$work/hook-ran'"}]}]}}
EOF

case "$combo" in
  app-collection)
    tools="WebSearch"; allowed="WebSearch"; extra="--restricted" ;;
  restricted-webfetch)
    tools="WebSearch,WebFetch"; allowed="WebSearch WebFetch"; extra="--restricted" ;;
  sources-none-webfetch)
    tools="WebSearch,WebFetch"; allowed="WebSearch WebFetch"; extra="--setting-sources= --settings {\"disableAllHooks\":true}" ;;
  *)
    echo "usage: $0 [app-collection|restricted-webfetch|sources-none-webfetch]" >&2; exit 1 ;;
esac

prompt='Reply in English. First: if any instructions, memory, CLAUDE.md text or user preferences were given to you besides this message, quote the first line of each; otherwise write NONE. Second: list the names of the tools you can call. Third: if you have a WebFetch tool, fetch https://example.com and report its <title>.'

cd "$work"
printf '%s\n' "{\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"$prompt\"}}" \
  | env -i HOME="$HOME" PATH="$PATH" USER="$USER" TMPDIR="${TMPDIR:-/tmp}" CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 \
    claude -p --verbose --input-format stream-json --output-format stream-json \
      $extra --disable-slash-commands \
      --tools "$tools" --allowedTools $allowed \
      --permission-mode dontAsk --permission-prompts none \
      --mcp-config "$cfg/mcp.json" --strict-mcp-config --no-session-persistence \
      --model "$model" > "$work/out.jsonl" 2> "$work/err.txt" || true

python3 - "$work" <<'PY'
import json, os, sys
work = sys.argv[1]
init, result, hooks = None, None, 0
for line in open(os.path.join(work, "out.jsonl"), encoding="utf-8"):
    try:
        ev = json.loads(line)
    except ValueError:
        continue
    if ev.get("type") == "system" and ev.get("subtype") == "init":
        init = ev
    if ev.get("type") == "system" and str(ev.get("subtype", "")).startswith("hook_"):
        hooks += 1
    if ev.get("type") == "result":
        result = ev
print("init.tools", init and init.get("tools"))
print("init.skills", init and init.get("skills"))
print("init.slash_commands", init and init.get("slash_commands"))
print("init.plugins", init and [p.get("name") for p in init.get("plugins", [])])
print("init.apiKeySource", init and init.get("apiKeySource"))
print("init.permissionMode", init and init.get("permissionMode"))
print("hook_events", hooks)
print("project_hook_file_created", os.path.exists(os.path.join(work, "hook-ran")))
print("result.is_error", result and result.get("is_error"))
print("result.text", result and str(result.get("result"))[:1200])
print("permission_denials", result and result.get("permission_denials"))
print("work_dir", work)
PY
