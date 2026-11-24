import { strict as assert } from "node:assert";
import {
  cpSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  MUTANTS_CONFIG_FILE,
  TARGETS_FILE,
  parseTargetsToml,
  targetProblems,
} from "../../mutants/check-targets.mjs";
import { repoRoot, scratch } from "./signature-repo.mjs";

const copyTargetsFixture = () => {
  const dir = scratch("h68-targets-");
  for (const rel of [
    TARGETS_FILE,
    MUTANTS_CONFIG_FILE,
    "crates/domain/src/money.rs",
    "crates/order-execution/src/lib.rs",
    "crates/adapters/store-sqlite/src/connection.rs",
  ]) {
    const dst = join(dir, rel);
    mkdirSync(join(dst, ".."), { recursive: true });
    cpSync(join(repoRoot, rel), dst);
  }
  return dir;
};

test("H68 mutation targets list is valid synced and rejects missing or drifted paths", () => {
  const entries = parseTargetsToml(
    readFileSync(join(repoRoot, TARGETS_FILE), "utf8"),
  );
  assert.ok(entries.length >= 100);
  assert.deepEqual(targetProblems(repoRoot), []);

  const missingPath = copyTargetsFixture();
  try {
    writeFileSync(
      join(missingPath, TARGETS_FILE),
      '[[target]]\npath = "crates/domain/src/does_not_exist.rs"\ncategory = "money_quantity_fx"\nreason = "없는 파일"\n',
    );
    writeFileSync(
      join(missingPath, MUTANTS_CONFIG_FILE),
      'test_tool = "nextest"\ncap_lints = true\nexamine_globs = [\n    "crates/domain/src/does_not_exist.rs",\n]\n',
    );
    assert.ok(
      targetProblems(missingPath).some((item) =>
        item.includes("존재하지 않는다"),
      ),
    );
  } finally {
    rmSync(missingPath, { recursive: true, force: true });
  }

  const drifted = copyTargetsFixture();
  try {
    writeFileSync(
      join(drifted, TARGETS_FILE),
      '[[target]]\npath = "crates/domain/src/money.rs"\ncategory = "money_quantity_fx"\nreason = "금액"\n',
    );
    writeFileSync(
      join(drifted, MUTANTS_CONFIG_FILE),
      'test_tool = "nextest"\ncap_lints = true\nexamine_globs = [\n    "crates/order-execution/src/**/*.rs",\n]\n',
    );
    assert.ok(
      targetProblems(drifted).some((item) =>
        item.includes("1:1로 일치하지 않는다"),
      ),
    );
  } finally {
    rmSync(drifted, { recursive: true, force: true });
  }

  const badCategory = copyTargetsFixture();
  try {
    writeFileSync(
      join(badCategory, TARGETS_FILE),
      '[[target]]\npath = "crates/domain/src/money.rs"\ncategory = "display"\nreason = "표시"\n',
    );
    writeFileSync(
      join(badCategory, MUTANTS_CONFIG_FILE),
      'test_tool = "nextest"\ncap_lints = true\nexamine_globs = [\n    "crates/domain/src/money.rs",\n]\n',
    );
    assert.ok(
      targetProblems(badCategory).some((item) =>
        item.includes("알 수 없는 분류"),
      ),
    );
  } finally {
    rmSync(badCategory, { recursive: true, force: true });
  }

  const duplicate = copyTargetsFixture();
  try {
    writeFileSync(
      join(duplicate, TARGETS_FILE),
      '[[target]]\npath = "crates/domain/src/money.rs"\ncategory = "money_quantity_fx"\nreason = "금액"\n\n[[target]]\npath = "crates/domain/src/money.rs"\ncategory = "money_quantity_fx"\nreason = "중복"\n',
    );
    writeFileSync(
      join(duplicate, MUTANTS_CONFIG_FILE),
      'test_tool = "nextest"\ncap_lints = true\nexamine_globs = [\n    "crates/domain/src/money.rs",\n    "crates/domain/src/money.rs",\n]\n',
    );
    assert.ok(
      targetProblems(duplicate).some((item) => item.includes("중복 경로")),
    );
  } finally {
    rmSync(duplicate, { recursive: true, force: true });
  }
});
