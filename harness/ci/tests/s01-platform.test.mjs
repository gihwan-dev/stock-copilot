import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

test("S01 mutation jobs execute the complete CommonCrypto catalog on macOS", () => {
  const workflow = JSON.parse(
    readFileSync(".github/workflows/nightly.yml", "utf8"),
  );
  for (const name of ["mutants", "mutants-daily"]) {
    const job = workflow.jobs[name];
    assert.equal(job["runs-on"], "macos-15");
    assert.equal(
      job.steps.find((step) => step.id === "mutation").run,
      name === "mutants"
        ? "just mutants-shard ${{ matrix.shard }}"
        : 'just mutants-daily "$MUTATION_BASE" "$MUTATION_SHARD"',
    );
  }
  const settings = readFileSync(".cargo/mutants.toml", "utf8");
  assert.match(settings, /test_tool = "nextest"/u);
  for (const required of [
    "crates/adapters/store-sqlite/src/connection.rs",
    "crates/adapters/store-sqlite/src/worker.rs",
    "crates/adapters/store-sqlite/src/proposal_consumption.rs",
    "crates/adapters/store-sqlite/src/validation_report.rs",
  ]) {
    assert.ok(settings.includes(`"${required}"`), required);
  }
  const nextest = readFileSync(".config/nextest.toml", "utf8");
  assert.match(nextest, /platform = "cfg\(target_os = \\"macos\\"\)"/u);
  assert.match(nextest, /run-wrapper = "network-sandbox"/u);
  assert.match(nextest, /deny network-outbound/u);
});
