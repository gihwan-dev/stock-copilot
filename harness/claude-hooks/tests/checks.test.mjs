import { strict as assert } from "node:assert";
import { spawnSync } from "node:child_process";
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { createRequire } from "node:module";
import { worktreesOf } from "../../../.claude/hooks/lib/project.mjs";
import { misplacedConfigs } from "../../config-files/check.mjs";
import { HASHED, HASHES_FILE, hashProblems } from "../hashes.mjs";
import { createFixture, realRules, repoRoot } from "./fixture.mjs";

const scratch = () =>
  realpathSync(mkdtempSync(join(tmpdir(), "claude-hooks-checks-")));

const place = (root, path, text = "x\n") => {
  mkdirSync(dirname(join(root, path)), { recursive: true });
  writeFileSync(join(root, path), text);
};

test("a nested ESLint config fails the config-files check of just check", () => {
  const root = scratch();
  [
    "apps/desktop/eslint.config.mjs",
    "apps/desktop/package.json",
    "crates/domain/Cargo.toml",
    "harness/arch-check/cases/x/Cargo.toml",
    "node_modules/x/package.json",
    "target/debug/x/Cargo.toml",
  ].forEach((path) => place(root, path));
  assert.deepEqual(misplacedConfigs(root, realRules), []);
  place(root, "apps/desktop/src/zz/eslint.config.mjs");
  place(root, "crates/domain/src/.prettierrc");
  assert.deepEqual(misplacedConfigs(root, realRules), [
    "apps/desktop/src/zz/eslint.config.mjs",
    "crates/domain/src/.prettierrc",
  ]);
  const run = spawnSync(
    process.execPath,
    [join(repoRoot, "harness/config-files/check.mjs"), root],
    { encoding: "utf8" },
  );
  assert.equal(run.status, 1, run.stdout);
  assert.match(run.stdout, /FAIL apps\/desktop\/src\/zz\/eslint\.config\.mjs/u);
});

test("the real repository has no misplaced config file", () => {
  assert.deepEqual(misplacedConfigs(repoRoot, realRules), []);
});

const copyHashed = () => {
  const root = scratch();
  cpSync(join(repoRoot, ".claude/hooks"), join(root, ".claude/hooks"), {
    recursive: true,
  });
  HASHED.forEach((path) => {
    mkdirSync(dirname(join(root, path)), { recursive: true });
    cpSync(join(repoRoot, path), join(root, path));
  });
  return root;
};

test("hook files, rules, allowlist and lock must match the protected hashes", () => {
  const expected = JSON.parse(
    readFileSync(join(repoRoot, HASHES_FILE), "utf8"),
  );
  assert.deepEqual(hashProblems(repoRoot, expected), []);
  [
    ".claude/hooks/guard-bash.mjs",
    "harness/claude-hooks/rules.json",
    "harness/agent-writable.txt",
    "harness/claude-hooks/lock.txt",
  ].forEach((path) => {
    const root = copyHashed();
    writeFileSync(join(root, path), "changed\n");
    assert.deepEqual(hashProblems(root, expected), [
      `${path}: 내용이 기대값과 다르다`,
    ]);
  });
  const added = copyHashed();
  place(added, ".claude/hooks/lib/extra.mjs");
  assert.deepEqual(hashProblems(added, expected), [
    ".claude/hooks/lib/extra.mjs: 기대값에 없는 파일이다",
  ]);
});

test("only genuine worktrees count as worktree roots", () => {
  const fixture = createFixture();
  const roots = worktreesOf(fixture.root);
  assert.ok(roots.includes(fixture.root));
  assert.ok(roots.includes(fixture.worktree));
  assert.ok(!roots.includes(join(fixture.base, "wx")));
});

test("just recipes do not see NODE_OPTIONS or NODE_PATH", () => {
  const root = scratch();
  writeFileSync(
    join(root, "justfile"),
    `import '${join(repoRoot, "justfile")}'\n\nprobe-node:\n    printf '[%s][%s]' "\${NODE_OPTIONS:-}" "\${NODE_PATH:-}"\n`,
  );
  const run = spawnSync(
    "just",
    ["--justfile", join(root, "justfile"), "probe-node"],
    {
      cwd: root,
      encoding: "utf8",
      env: {
        ...process.env,
        NODE_OPTIONS: "--require=/tmp/x.js",
        NODE_PATH: "/tmp",
      },
    },
  );
  assert.equal(run.status, 0, run.stderr);
  assert.equal(run.stdout, "[][]");
});

const { ESLint } = createRequire(join(repoRoot, "apps/desktop/package.json"))(
  "eslint",
);

test("hook scripts allow no comment except the 2.1 directive form", async () => {
  const linter = new ESLint({
    cwd: repoRoot,
    overrideConfigFile: join(
      repoRoot,
      "harness/claude-hooks/eslint.config.mjs",
    ),
    cache: false,
  });
  const rules = async (code) => {
    const [result] = await linter.lintText(code, {
      filePath: join(repoRoot, ".claude/hooks/probe.mjs"),
    });
    return [...new Set(result.messages.map((item) => item.ruleId))].sort();
  };
  assert.deepEqual(await rules("// note\nexport const a = 1;\n"), [
    "local/no-comments",
  ]);
  assert.deepEqual(await rules("/* note */\nexport const a = 1;\n"), [
    "local/no-comments",
  ]);
  assert.deepEqual(
    await rules(
      "export const a = () => {\n  // eslint-disable-next-line no-debugger -- 예제\n  debugger;\n};\n",
    ),
    [],
  );
  assert.deepEqual(
    await rules(
      "// eslint-disable-next-line local/no-comments -- 끄기\n// note\nexport const a = 1;\n",
    ),
    ["@eslint-community/eslint-comments/no-restricted-disable"],
  );
});

test("harness and test scripts allow no comment except the 2.1 directive form", async () => {
  const linter = new ESLint({
    cwd: repoRoot,
    overrideConfigFile: join(
      repoRoot,
      "apps/desktop/eslint.scripts.config.mjs",
    ),
    cache: false,
  });
  const rules = async (path, code) => {
    const [result] = await linter.lintText(code, {
      filePath: join(repoRoot, path),
    });
    return [...new Set(result.messages.map((item) => item.ruleId))].sort();
  };
  for (const path of ["harness/x/check.mjs", "apps/desktop/tests/a.test.mjs"]) {
    assert.deepEqual(await rules(path, "// note\nexport const a = 1;\n"), [
      "local/no-comments",
    ]);
    assert.deepEqual(
      await rules(
        path,
        "export const a = () => {\n  // eslint-disable-next-line no-debugger -- 예제\n  debugger;\n};\n",
      ),
      [],
    );
    assert.deepEqual(
      await rules(
        path,
        "// eslint-disable-next-line local/no-comments -- 끄기\n// note\nexport const a = 1;\n",
      ),
      ["@eslint-community/eslint-comments/no-restricted-disable"],
    );
  }
});
