import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import {
  graphProblems,
  linkProblems,
} from "../../../harness/sqlcipher/rules.mjs";
import { repoRoot } from "./paths.mjs";

test("SQLCipher rejects missing pins and OpenSSL graph or linkage", () => {
  const cases = JSON.parse(
    readFileSync(join(repoRoot, "harness/sqlcipher/cases.json"), "utf8"),
  );
  for (const entry of cases.graphs)
    assert.deepEqual(graphProblems(entry.input), entry.expected);
  for (const entry of cases.links)
    assert.deepEqual(linkProblems(entry.input), entry.expected);
});
