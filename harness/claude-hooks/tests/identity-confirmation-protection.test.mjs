import { strict as assert } from "node:assert";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { isWritable } from "../../../.claude/hooks/lib/allowlist.mjs";
import { currentHashes } from "../hashes.mjs";
import { realAllowlist, repoRoot } from "./fixture.mjs";
import {
  FRAGMENT_FILE,
  deniedBy,
  readAllowlistFile,
} from "../../sandbox/fragment.mjs";

const PROTECTED_FAKE =
  "crates/testkit-ports/src/scripted_identity_confirmation.rs";

test("ScriptedIdentityConfirmation is protected in allowlist, sandbox fragment, Codex config, and hashes", () => {
  assert.equal(isWritable(realAllowlist, PROTECTED_FAKE), false);
  assert.equal(isWritable(readAllowlistFile(repoRoot), PROTECTED_FAKE), false);
  const fragment = JSON.parse(
    readFileSync(join(repoRoot, FRAGMENT_FILE), "utf8"),
  );
  assert.equal(
    deniedBy(fragment.sandbox.filesystem.denyWrite, PROTECTED_FAKE),
    true,
  );
  const codexConfig = readFileSync(
    join(repoRoot, ".codex/config.toml"),
    "utf8",
  );
  assert.ok(codexConfig.includes(`"${PROTECTED_FAKE}" = "read"`));
  const expected = createHash("sha256")
    .update(readFileSync(join(repoRoot, PROTECTED_FAKE)))
    .digest("hex");
  const recorded = JSON.parse(
    readFileSync(join(repoRoot, "harness/claude-hooks/hashes.json"), "utf8"),
  );
  assert.equal(recorded[PROTECTED_FAKE], expected);
  assert.equal(currentHashes(repoRoot)[PROTECTED_FAKE], expected);
});
