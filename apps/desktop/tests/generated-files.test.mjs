import { strict as assert } from "node:assert";
import { cpSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import {
  compareGenerated,
  desktopTypeScriptPaths,
  fixedPart,
  fixedPartProblems,
  generateInto,
  generatedPaths,
  readFixed,
  readList,
  repoRoot,
  scratchRoot,
} from "../../../harness/generated-files/generated.mjs";
import { desktopRoot } from "./paths.mjs";

const { ESLint } = createRequire(join(desktopRoot, "package.json"))("eslint");
const appLinter = new ESLint({ cwd: desktopRoot });

const expectedList = [
  ["apps/desktop/src-tauri/ipc/commands.txt", "none", undefined],
  ["apps/desktop/src-tauri/permissions/commands.toml", "none", undefined],
  [
    "apps/desktop/src/shared/ipc/bindings.ts",
    "prettier",
    ["@tauri-apps/api/core"],
  ],
  [
    "apps/desktop/src/shared/ipc/testing/mockBindings.ts",
    "prettier",
    ["@tauri-apps/api/core", "@tauri-apps/api/mocks"],
  ],
  ["apps/desktop/src/shared/ipc/testing/responses.ts", "prettier", []],
];

const lintGenerated = async (path, text) => {
  const [result] = await appLinter.lintText(text, {
    filePath: join(desktopRoot, path),
  });
  return result.messages.map(
    (message) => `${message.ruleId} :: ${message.message}`,
  );
};

const committed = (path) => readFileSync(join(desktopRoot, path), "utf8");

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
    list.files.map((entry) => [entry.path, entry.format, entry.imports]),
    expectedList,
  );
  assert.deepEqual(
    list.files.filter(
      (entry) =>
        Object.keys(entry).length !== (entry.imports === undefined ? 2 : 3),
    ),
    [],
  );
});

test("generated TypeScript files get only the generated rule bundle with inline config off", async () => {
  for (const path of desktopTypeScriptPaths()) {
    assert.equal(await appLinter.isPathIgnored(join(desktopRoot, path)), false);
    const config = await appLinter.calculateConfigForFile(
      join(desktopRoot, path),
    );
    assert.equal(config.linterOptions.noInlineConfig, true, path);
    assert.equal(
      config.rules["@typescript-eslint/no-explicit-any"][0],
      0,
      path,
    );
    assert.equal(config.rules["no-restricted-globals"][0], 2, path);
    assert.deepEqual(await lintGenerated(path, committed(path)), [], path);
  }
  for (const neighbour of [
    "src/shared/ipc/call.ts",
    "src/shared/ipc/schema.ts",
    "src/shared/ipc/index.ts",
    "src/shared/ipc/testing/fakes.ts",
  ]) {
    const config = await appLinter.calculateConfigForFile(
      join(desktopRoot, neighbour),
    );
    assert.equal(config.linterOptions.noInlineConfig, false, neighbour);
    assert.equal(
      config.rules["@typescript-eslint/no-explicit-any"][0],
      2,
      neighbour,
    );
  }
});

test("a header that runs code in bindings.ts fails the generated rule bundle", async () => {
  const path = "src/shared/ipc/bindings.ts";
  const found = await lintGenerated(
    path,
    `void fetch("https://example.invalid");\n${committed(path)}`,
  );
  assert.ok(
    found.includes("no-restricted-globals :: Unexpected use of 'fetch'."),
    `${found}`,
  );
  assert.ok(
    found.includes(
      "no-restricted-syntax :: 생성물: 최상위에는 가져오기와 선언만 둔다.",
    ),
    `${found}`,
  );
});

test("code injected through a recorded response fails the generated rule bundle", async () => {
  const path = "src/shared/ipc/testing/responses.ts";
  const found = await lintGenerated(
    path,
    'export const responses = { appVersion: 0, injected: (() => { void fetch("https://example.invalid"); return 1 as any; })() } as const;\n',
  );
  assert.ok(
    found.includes("no-restricted-globals :: Unexpected use of 'fetch'."),
    `${found}`,
  );
  assert.ok(
    found.includes(
      "no-restricted-syntax :: 생성물: 즉시 실행 함수를 쓰지 않는다.",
    ),
    `${found}`,
  );
});

test("generated files may import only their listed modules and nothing dynamically", async () => {
  const found = await lintGenerated(
    "src/shared/ipc/bindings.ts",
    'import { mockIPC } from "@tauri-apps/api/mocks";\nexport const load = () => import("./other");\nexport { x } from "./other";\nexport const mock = mockIPC;\n',
  );
  assert.deepEqual(found.sort(), [
    "no-restricted-syntax :: 생성물: 가져올 수 있는 모듈은 @tauri-apps/api/core뿐이다.",
    "no-restricted-syntax :: 생성물: 다른 모듈을 다시 내보내지 않는다.",
    "no-restricted-syntax :: 생성물: 동적 import()를 쓰지 않는다.",
    "no-restricted-syntax :: 생성물: 최상위에는 가져오기와 선언만 둔다.",
  ]);
  const responses = await lintGenerated(
    "src/shared/ipc/testing/responses.ts",
    'import { invoke } from "@tauri-apps/api/core";\nexport const responses = { invoke } as const;\n',
  );
  assert.deepEqual(responses, [
    "no-restricted-syntax :: 생성물: 다른 모듈을 가져오지 않는다.",
  ]);
});

test("inline directives do not switch off the generated rule bundle", async () => {
  const found = await lintGenerated(
    "src/shared/ipc/bindings.ts",
    '// eslint-disable-next-line no-restricted-globals -- 우회\nexport const leak = () => fetch("https://example.invalid");\n',
  );
  assert.ok(
    found.includes("no-restricted-globals :: Unexpected use of 'fetch'."),
    `${found}`,
  );
});

test("the fixed header and runtime of the generator match harness/generated-files/fixed.json", () => {
  assert.deepEqual(fixedPartProblems(), []);
  const fixed = readFixed();
  assert.deepEqual(Object.keys(fixed).sort(), [
    "apps/desktop/src/shared/ipc/bindings.ts",
    "apps/desktop/src/shared/ipc/testing/mockBindings.ts",
  ]);
  for (const [path, parts] of Object.entries(fixed)) {
    for (const part of Object.keys(parts)) {
      const copy = copyOfGenerated();
      const text = readFileSync(join(copy, path), "utf8");
      writeFileSync(
        join(copy, path),
        text.replace(
          fixedPart(text, part),
          `${fixedPart(text, part)}void 0;\n`,
        ),
      );
      assert.deepEqual(fixedPartProblems(copy), [
        `${path}: the ${part} differs from harness/generated-files/fixed.json`,
      ]);
    }
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
