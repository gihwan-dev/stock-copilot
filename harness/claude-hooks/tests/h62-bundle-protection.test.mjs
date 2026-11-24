import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { isWritable } from "../../../.claude/hooks/lib/allowlist.mjs";
import { decideEdit } from "../../../.claude/hooks/lib/edit-guard.mjs";
import { LOCK } from "../../../.claude/hooks/lib/project.mjs";
import { allowedPath } from "../../ci/merge-gate.mjs";
import { deniedBy, FRAGMENT_FILE } from "../../sandbox/fragment.mjs";
import { PROTECTED } from "../../codex-hooks/config.mjs";
import {
  createFixture,
  projectFor,
  readJson,
  realAllowlist,
  repoRoot,
  roleInput,
  ruleSet,
} from "./fixture.mjs";

const BUNDLE_PATH = "harness/test-diff/bundles/D-56.toml";
const APPROVALS_PATH = "harness/test-diff/approvals.toml";

test("승인 묶음 경로는 허용 목록·샌드박스·Codex·병합 게이트에서 보호된다", () => {
  assert.equal(isWritable(realAllowlist, BUNDLE_PATH), false);
  const fragment = readJson(FRAGMENT_FILE);
  assert.equal(
    deniedBy(fragment.sandbox.filesystem.denyWrite, BUNDLE_PATH),
    true,
  );
  assert.ok(PROTECTED.includes("harness"));
  assert.equal(allowedPath(BUNDLE_PATH), false);
  const codex = readFileSync(join(repoRoot, ".codex/config.toml"), "utf8");
  assert.match(codex, /"harness" = "read"/u);
});

test("승인 묶음 경로는 approvals.toml과 동일한 보호 규칙으로 모든 역할과 잠금 상태에 적용된다", () => {
  const fixture = createFixture();
  const check = (relativePath, role, lock) => {
    const verdict = decideEdit(
      {
        hook_event_name: "PreToolUse",
        tool_name: "Write",
        tool_input: {
          file_path: join(fixture.root, relativePath),
          content: 'task = "D-56"\n',
        },
        cwd: fixture.root,
        ...roleInput(role),
      },
      projectFor(fixture, { lock }),
    );
    return { decision: verdict.decision, rules: ruleSet(verdict) };
  };
  for (const role of [
    "implementer",
    "adversarial-reviewer",
    "review-critic",
    undefined,
  ]) {
    for (const lock of [LOCK.off, LOCK.on]) {
      assert.deepEqual(
        check(BUNDLE_PATH, role, lock),
        check(APPROVALS_PATH, role, lock),
        `${role ?? "main"} lock=${lock}`,
      );
    }
  }
  assert.deepEqual(check(BUNDLE_PATH, "implementer", LOCK.off), {
    decision: "warn",
    rules: ["protected-path"],
  });
  assert.deepEqual(check(BUNDLE_PATH, "implementer", LOCK.on), {
    decision: "deny",
    rules: ["protected-path"],
  });
  assert.deepEqual(check(BUNDLE_PATH, "adversarial-reviewer", LOCK.off), {
    decision: "deny",
    rules: ["role-without-edits"],
  });
  assert.deepEqual(check(BUNDLE_PATH, "review-critic", LOCK.off), {
    decision: "deny",
    rules: ["role-without-edits"],
  });
  assert.deepEqual(check(BUNDLE_PATH, undefined, LOCK.off), {
    decision: "warn",
    rules: ["protected-path"],
  });
  assert.deepEqual(check(BUNDLE_PATH, undefined, LOCK.on), {
    decision: "deny",
    rules: ["protected-path"],
  });
});
