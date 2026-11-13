import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { configText, hooksConfig } from "../config.mjs";
import {
  dataVariants,
  applyVariant,
  repoRoot,
} from "../../claude-hooks/tests/fixture.mjs";
import { isWritable } from "../../../.claude/hooks/lib/allowlist.mjs";
import { loadProject } from "../../../.claude/hooks/lib/project.mjs";

const policy = JSON.parse(
  readFileSync(new URL("../../sandbox/policy.json", import.meta.url), "utf8"),
);

test("Codex config preserves supported sandbox policy entries and pins the shell PATH", () => {
  const text = configText(policy, "/example", "/node");
  assert.match(text, /PATH = "\/example\/Library\/pnpm:\/node:/u);
  assert.match(text, /allow_login_shell = false/u);
  assert.match(text, /shell_snapshot = false/u);
  assert.match(text, /"\/example\/\.ssh" = "deny"/u);
  assert.match(text, /network_proxy = true/u);
  for (const domain of policy.base.network.allowedDomains)
    assert.ok(text.includes(`${JSON.stringify(domain)} = "allow"`));
  const variants = dataVariants(policy).filter(
    (variant) =>
      [
        "network.allowedDomains",
        "network.allowUnixSockets",
        "filesystem.allowWrite",
        "filesystem.denyWrite",
        "filesystem.denyRead",
      ].some((key) => variant.path.join(".") === `base.${key}`) &&
      !String(variant.name).includes("*"),
  );
  for (const variant of variants) {
    const changed = applyVariant(policy, variant);
    if (JSON.stringify(changed) === JSON.stringify(policy)) continue;
    const path = variant.name;
    if (
      path.includes("../") ||
      (policy.base.filesystem.denyRead.some((entry) =>
        path.includes(JSON.stringify(entry)),
      ) &&
        policy.base.filesystem.denyWrite.some((entry) =>
          path.includes(JSON.stringify(entry)),
        ))
    )
      continue;
    assert.notEqual(configText(changed, "/example", "/node"), text, path);
  }
});

test("Codex hook events cover pre post and stop and all config paths are protected", () => {
  const config = hooksConfig();
  assert.deepEqual(Object.keys(config.hooks), [
    "PreToolUse",
    "PostToolUse",
    "Stop",
  ]);
  for (const event of Object.keys(config.hooks)) {
    const changed = structuredClone(config);
    delete changed.hooks[event];
    assert.notDeepEqual(changed, config);
  }
  const project = loadProject(repoRoot);
  for (const path of [
    ".codex/config.toml",
    ".codex/hooks.json",
    "harness/codex-hooks/dispatch.mjs",
  ])
    assert.equal(isWritable(project.allowlist, path), false);
});
