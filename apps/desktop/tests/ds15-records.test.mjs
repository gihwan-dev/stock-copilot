import { createHash } from "node:crypto";
import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  mkdirSync,
  readFileSync,
  readdirSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { mkdtempSync } from "../../../harness/ci/test-temp.mjs";
import { join } from "node:path";
import { settings } from "../../../harness/visual/policy.mjs";
import {
  recordProblems,
  storedProblems,
  writeBaselineSet,
} from "../../../harness/visual/records.mjs";
import { baselineFixture } from "./ds15-records-fixture.mjs";
import { repoRoot } from "./paths.mjs";
const scratch = () => {
  mkdirSync(join(repoRoot, "target"), { recursive: true });
  return mkdtempSync(join(repoRoot, "target/DS-15-records-"));
};

test("DS15 measurement requires exact scope and both hashes for distinct files", () => {
  const { images, measurements } = baselineFixture();
  const record = { settings, measurements };
  assert.deepEqual(recordProblems(record, images), []);
  assert.ok(recordProblems(undefined, images).length > 0);
  assert.ok(
    recordProblems({ ...record, settings: { ...settings, scale: 2 } }, images)
      .length > 0,
  );
  assert.ok(
    recordProblems({ ...record, measurements: measurements.slice(1) }, images)
      .length > 0,
  );
  assert.ok(
    recordProblems(
      {
        ...record,
        measurements: [measurements[0], ...measurements.slice(0, -1)],
      },
      images,
    ).length > 0,
  );
  assert.ok(
    recordProblems(
      { ...record, measurements: [...measurements, { component: "Ghost" }] },
      images,
    ).length > 0,
  );
  assert.ok(
    recordProblems({ ...record, unexpected: "extra" }, images).length > 0,
  );
  const emptyRecord = structuredClone(record);
  const emptyHash = createHash("sha256").update(Buffer.alloc(0)).digest("hex");
  emptyRecord.measurements[0] = {
    ...emptyRecord.measurements[0],
    bytes: 0,
    first: emptyHash,
    second: emptyHash,
  };
  assert.ok(
    recordProblems(emptyRecord, {
      ...images,
      [Object.keys(images)[0]]: Buffer.alloc(0),
    }).length > 0,
  );
  for (const field of ["bytes", "first", "second"]) {
    const invalid = structuredClone(record);
    invalid.measurements[1][field] = field === "bytes" ? 1 : "wrong";
    assert.ok(recordProblems(invalid, images).length > 0, field);
  }
  const missing = { ...images };
  delete missing[Object.keys(missing)[2]];
  assert.ok(recordProblems(record, missing).length > 0);
  assert.ok(
    recordProblems(record, {
      ...images,
      "Deleted-light-chromium-linux-arm64.png": Buffer.from("extra"),
    }).length > 0,
  );
  const swapped = { ...images };
  const [a, b] = Object.keys(images);
  [swapped[a], swapped[b]] = [swapped[b], swapped[a]];
  assert.ok(recordProblems(record, swapped).length > 0);
});

test("DS15 update writes matching measurement and removes obsolete generated images", async () => {
  const folder = scratch();
  try {
    const { images, measurements } = baselineFixture();
    writeFileSync(
      join(folder, "Deleted-light-chromium-linux-arm64.png"),
      "obsolete",
    );
    await writeBaselineSet(folder, measurements, images);
    assert.deepEqual(storedProblems(folder), []);
    assert.deepEqual(
      JSON.parse(readFileSync(join(folder, "measurement.json"), "utf8")),
      { settings, measurements },
    );
    assert.deepEqual(
      readdirSync(folder).sort(),
      [...Object.keys(images), "measurement.json"].sort(),
    );
    for (const [name, bytes] of Object.entries(images))
      assert.deepEqual(readFileSync(join(folder, name)), bytes);
    writeFileSync(join(folder, "measurement.json"), "invalid JSON");
    assert.ok(storedProblems(folder).length > 0);
    await writeBaselineSet(folder, measurements, images);
    assert.deepEqual(storedProblems(folder), []);
    writeFileSync(join(folder, Object.keys(images)[0]), "corrupt");
    assert.ok(storedProblems(folder).length > 0);
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});

test("DS15 comparison rejects stale extras and update refuses foreign or unmanaged files", async () => {
  const folder = scratch();
  try {
    const { images, measurements } = baselineFixture();
    await writeBaselineSet(folder, measurements, images);
    const original = readFileSync(join(folder, "measurement.json"));
    for (const name of [
      "Button-light-chromium-darwin-arm64.png",
      "Button-darwin-chromium-linux-arm64.png",
      "unknown.txt",
    ]) {
      writeFileSync(join(folder, name), "extra");
      assert.ok(storedProblems(folder).length > 0);
      await assert.rejects(writeBaselineSet(folder, measurements, images));
      assert.deepEqual(
        readFileSync(join(folder, "measurement.json")),
        original,
      );
      rmSync(join(folder, name));
    }
    writeFileSync(
      join(folder, "Deleted-light-chromium-linux-arm64.png"),
      "extra",
    );
    assert.ok(storedProblems(folder).length > 0);
    await writeBaselineSet(folder, measurements, images);
    assert.deepEqual(storedProblems(folder), []);
  } finally {
    rmSync(folder, { recursive: true, force: true });
  }
});
