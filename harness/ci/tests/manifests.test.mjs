import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  PACKAGE_DEPENDENCY_KEYS,
  manifestKind,
  packageChangesOutsideDependencies,
} from "../signatures/manifests.mjs";

const BASE = {
  name: "desktop",
  private: true,
  scripts: { build: "vite build" },
  dependencies: { react: "19.2.0" },
};

const text = (value) => `${JSON.stringify(value, undefined, 2)}\n`;

test("each package.json dependency list may change without a signature", () => {
  assert.deepEqual(PACKAGE_DEPENDENCY_KEYS, [
    "dependencies",
    "devDependencies",
    "optionalDependencies",
    "peerDependencies",
  ]);
  PACKAGE_DEPENDENCY_KEYS.forEach((key) =>
    assert.equal(
      packageChangesOutsideDependencies(
        text(BASE),
        text({ ...BASE, [key]: { zod: "4.6.7" } }),
      ),
      false,
      key,
    ),
  );
  assert.equal(
    packageChangesOutsideDependencies(text(BASE), text(BASE)),
    false,
  );
  assert.equal(packageChangesOutsideDependencies(undefined, undefined), false);
});

test("every other package.json change needs a signature", () => {
  [
    { ...BASE, scripts: { build: "true" } },
    { ...BASE, pnpm: { overrides: { react: "1.0.0" } } },
    { ...BASE, name: "other" },
  ].forEach((changed) =>
    assert.equal(
      packageChangesOutsideDependencies(text(BASE), text(changed)),
      true,
      JSON.stringify(changed),
    ),
  );
  assert.equal(packageChangesOutsideDependencies(text(BASE), "{"), true);
  assert.equal(packageChangesOutsideDependencies(text(BASE), "[]\n"), true);
  assert.equal(packageChangesOutsideDependencies(undefined, text(BASE)), true);
  assert.equal(packageChangesOutsideDependencies(text(BASE), undefined), true);
});

test("only Cargo.toml and package.json are dependency manifests", () => {
  assert.equal(manifestKind("crates/domain/Cargo.toml"), "cargo");
  assert.equal(manifestKind("apps/desktop/package.json"), "package");
  assert.equal(manifestKind("Cargo.lock"), undefined);
  assert.equal(manifestKind("crates/domain/src/package.json.rs"), undefined);
});
