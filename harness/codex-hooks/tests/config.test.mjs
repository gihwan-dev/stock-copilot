import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { configText, hooksConfig, policyCoverage } from "../config.mjs";
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
  const rendered = (value) =>
    configText(value) + JSON.stringify(policyCoverage(value));
  const variants = dataVariants(policy).filter(
    (variant) =>
      variant.path.join(".") === "base.network.allowedDomains" ||
      variant.path.join(".") === "base.network.allowUnixSockets" ||
      variant.path.slice(0, 2).join(".") === "base.filesystem",
  );
  for (const variant of variants)
    assert.notEqual(
      rendered(applyVariant(policy, variant)),
      rendered(policy),
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
