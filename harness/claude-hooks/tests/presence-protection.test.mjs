import { strict as assert } from "node:assert";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { currentHashes } from "../hashes.mjs";
import { repoRoot } from "./fixture.mjs";

test("ScriptedPresence is included in protected hashes with its actual content", () => {
  const path = "crates/testkit-ports/src/scripted_presence.rs";
  const expected = createHash("sha256")
    .update(readFileSync(join(repoRoot, path)))
    .digest("hex");
  assert.equal(currentHashes(repoRoot)[path], expected);
});
