import { strict as assert } from "node:assert";
import { cpSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  compareGenerated,
  desktopTypeScriptPaths,
  generateInto,
  generatedPaths,
  readList,
  repoRoot,
  scratchRoot,
} from "../../../harness/generated-files/generated.mjs";
import { desktopRoot } from "./paths.mjs";

const { ESLint } = createRequire(join(desktopRoot, "package.json"))("eslint");
const appLinter = new ESLint({ cwd: desktopRoot });

const expectedList = [
  ["apps/desktop/src-tauri/ipc/commands.txt", "none"],
  ["apps/desktop/src-tauri/permissions/commands.toml", "none"],
  ["apps/desktop/src/shared/ipc/bindings.ts", "prettier"],
  ["apps/desktop/src/shared/ipc/testing/mockBindings.ts", "prettier"],
  ["apps/desktop/src/shared/ipc/testing/responses.ts", "prettier"],
];

const copyOfGenerated = () => {
  const root = mkdtempSync(join(tmpdir(), "generated-copy-"));
  for (const path of generatedPaths()) {
    cpSync(join(repoRoot, path), join(root, path));
  }
  return root;
};

test("the generated file list holds exactly the IPC outputs of gen-bindings", () => {
  const list = readList();
  assert.deepEqual(Object.keys(list).sort(), ["files", "reason"]);
  assert.ok(list.reason.length > 0);
  assert.deepEqual(
    list.files.map((entry) => [entry.path, entry.format]),
    expectedList,
  );
  assert.deepEqual(
    list.files.filter((entry) => Object.keys(entry).length !== 2),
    [],
  );
});

test("ESLint ignores exactly the generated TypeScript files and keeps their neighbours", async () => {
  for (const path of desktopTypeScriptPaths()) {
    assert.equal(await appLinter.isPathIgnored(join(desktopRoot, path)), true);
  }
  for (const neighbour of [
    "src/shared/ipc/call.ts",
    "src/shared/ipc/schema.ts",
    "src/shared/ipc/index.ts",
    "src/shared/ipc/testing/fakes.ts",
    "src/shared/ipc/bindings.test.ts",
    "src/shared/ipc/testing/other.ts",
  ]) {
    assert.equal(
      await appLinter.isPathIgnored(join(desktopRoot, neighbour)),
      false,
      neighbour,
    );
  }
});

test("knip relaxes only export and type issues and only in the generated TypeScript files", () => {
  const knip = JSON.parse(readFileSync(join(desktopRoot, "knip.json"), "utf8"));
  assert.deepEqual(
    Object.keys(knip.ignoreIssues).sort(),
    desktopTypeScriptPaths().sort(),
  );
  assert.deepEqual(
    Object.values(knip.ignoreIssues).filter(
      (types) => JSON.stringify(types) !== JSON.stringify(["exports", "types"]),
    ),
    [],
  );
  assert.equal(knip.ignore, undefined);
  assert.equal(knip.ignoreFiles, undefined);
});

test("just check regenerates the outputs and gen-bindings writes them", () => {
  const justfile = readFileSync(join(repoRoot, "justfile"), "utf8");
  assert.match(justfile, /^ {4}node harness\/generated-files\/check\.mjs$/m);
  assert.match(
    justfile,
    /^gen-bindings:\n {4}node harness\/generated-files\/check\.mjs --write$/m,
  );
});

test("an untouched copy of the committed outputs compares clean", () => {
  assert.deepEqual(compareGenerated(copyOfGenerated()), []);
});

test("a hand edit of any generated file is reported", () => {
  for (const path of generatedPaths()) {
    const edited = copyOfGenerated();
    writeFileSync(
      join(edited, path),
      `${readFileSync(join(edited, path), "utf8")}\n`,
    );
    assert.deepEqual(compareGenerated(edited), [
      `${path}: differs from regenerated output; run just gen-bindings`,
    ]);
  }
});

test("a generated file missing from the repository is reported", () => {
  const fresh = copyOfGenerated();
  const empty = mkdtempSync(join(tmpdir(), "generated-empty-"));
  assert.equal(compareGenerated(fresh, empty).length, generatedPaths().length);
});

test("regenerating from the current Rust source reproduces the committed files", () => {
  const outRoot = scratchRoot("test");
  assert.deepEqual(generateInto(outRoot), []);
  assert.deepEqual(compareGenerated(outRoot), []);
});
