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
  assert.doesNotMatch(text, /\/example|\/node/u);
  assert.match(text, /allow_login_shell = false/u);
  assert.match(text, /shell_snapshot = false/u);
  assert.match(text, /"~\/\.ssh" = "deny"/u);
  assert.match(text, /network_proxy = true/u);
  assert.match(text, /allow_local_binding = true/u);
  const withoutBinding = structuredClone(policy);
  withoutBinding.base.network.allowLocalBinding = false;
  assert.match(configText(withoutBinding), /allow_local_binding = false/u);
  for (const domain of policy.base.network.allowedDomains)
    assert.ok(text.includes(`${JSON.stringify(domain)} = "allow"`));
  const variants = dataVariants(policy).filter((variant) => {
    const name = variant.path.join(".");
    const removed = JSON.parse(
      variant.name.slice(variant.name.indexOf("=") + 1),
    );
    return (
      name === "base.network.allowedDomains" ||
      (name === "base.filesystem.allowWrite" &&
        removed.startsWith("~/") &&
        !removed.includes("*")) ||
      (name === "base.filesystem.denyRead" && !removed.includes("*")) ||
      (name === "base.filesystem.denyWrite" &&
        removed.startsWith("~/") &&
        !removed.includes("*") &&
        !policy.base.filesystem.denyRead.includes(removed))
    );
  });
  for (const variant of variants)
    assert.notEqual(
      configText(applyVariant(policy, variant)),
      text,
      variant.name,
    );
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
