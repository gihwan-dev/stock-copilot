import { strict as assert } from "node:assert";
import { test } from "node:test";
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readlinkSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { mkdtempSync } from "../test-temp.mjs";
import {
  applyIndexToClone,
  commitIndexSnapshot,
  historicalMasterRef,
} from "../index-clone.mjs";

const env = {
  ...Object.fromEntries(
    Object.entries(process.env).filter(([key]) => !key.startsWith("GIT_")),
  ),
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_AUTHOR_NAME: "Fixture",
  GIT_AUTHOR_EMAIL: "fixture@example.invalid",
  GIT_COMMITTER_NAME: "Fixture",
  GIT_COMMITTER_EMAIL: "fixture@example.invalid",
};
const git = (cwd, ...args) =>
  execFileSync("git", args, { cwd, env, stdio: "pipe" });
const write = (root, path, body) => {
  const destination = join(root, path);
  mkdirSync(dirname(destination), { recursive: true });
  writeFileSync(destination, body);
};

test("복제본은 HEAD가 아니라 현재 색인의 새 파일·변경·삭제를 사용한다", () => {
  const parent = mkdtempSync(join(tmpdir(), "h57-index-"));
  const source = join(parent, "source");
  const clone = join(parent, "clone");
  mkdirSync(source);
  git(source, "init", "--quiet", "-b", "master");
  write(source, "entry.mjs", "export default 'old';\n");
  write(source, "deleted.txt", "old\n");
  git(source, "add", ".");
  git(source, "commit", "--quiet", "-m", "base");
  git(source, "clone", "--quiet", "--shared", ".", clone);
  write(
    source,
    "entry.mjs",
    "export { value as default } from './new-helper.mjs';\n",
  );
  write(source, "new-helper.mjs", "export const value = 'staged';\n");
  git(source, "rm", "--quiet", "deleted.txt");
  git(source, "add", "entry.mjs", "new-helper.mjs");
  write(source, "new-helper.mjs", "export const value = 'unstaged';\n");
  write(source, "untracked.txt", "untracked\n");
  applyIndexToClone(source, clone);
  assert.equal(
    readFileSync(join(clone, "new-helper.mjs"), "utf8"),
    "export const value = 'staged';\n",
  );
  assert.equal(existsSync(join(clone, "deleted.txt")), false);
  assert.equal(existsSync(join(clone, "untracked.txt")), false);
  assert.deepEqual(
    git(clone, "diff", "--cached", "--name-only").toString().trim().split("\n"),
    ["deleted.txt", "entry.mjs", "new-helper.mjs"],
  );
  assert.equal(
    execFileSync(
      process.execPath,
      [
        "--input-type=module",
        "-e",
        "import value from './entry.mjs'; if (value !== 'staged') process.exit(1)",
      ],
      { cwd: clone, env },
    ).length,
    0,
  );
});

test("별도 색인을 지정하면 그 색인만 복제하고 원본 색인은 그대로 둔다", () => {
  const parent = mkdtempSync(join(tmpdir(), "h57-alt-index-"));
  const source = join(parent, "source");
  const clone = join(parent, "clone");
  const index = join(parent, "alternate-index");
  mkdirSync(source);
  git(source, "init", "--quiet", "-b", "master");
  write(source, "existing.txt", "base\n");
  git(source, "add", ".");
  git(source, "commit", "--quiet", "-m", "base");
  git(source, "clone", "--quiet", "--shared", ".", clone);
  const alternateEnv = { ...env, GIT_INDEX_FILE: index };
  execFileSync("git", ["read-tree", "HEAD"], {
    cwd: source,
    env: alternateEnv,
  });
  write(source, "new-helper.mjs", "export const value = 'alternate';\n");
  execFileSync("git", ["add", "new-helper.mjs"], {
    cwd: source,
    env: alternateEnv,
  });
  const original = git(source, "diff", "--cached", "--name-only").toString();
  applyIndexToClone(source, clone, index);
  assert.equal(
    readFileSync(join(clone, "new-helper.mjs"), "utf8"),
    "export const value = 'alternate';\n",
  );
  assert.equal(
    git(source, "diff", "--cached", "--name-only").toString(),
    original,
  );
  assert.equal(
    execFileSync("git", ["diff", "--cached", "--name-only"], {
      cwd: source,
      env: alternateEnv,
    }).toString(),
    "new-helper.mjs\n",
  );
});

test("임시 커밋을 써도 이전 master 이력의 기준이 바뀌지 않는다", () => {
  const parent = mkdtempSync(join(tmpdir(), "h57-history-"));
  const source = join(parent, "source");
  const clone = join(parent, "clone");
  mkdirSync(source);
  git(source, "init", "--quiet", "-b", "master");
  write(source, "version.txt", "one\n");
  git(source, "add", ".");
  git(source, "commit", "--quiet", "-m", "first");
  const previous = git(source, "rev-parse", "HEAD").toString().trim();
  write(source, "version.txt", "two\n");
  git(source, "add", ".");
  git(source, "commit", "--quiet", "-m", "second");
  const original = git(source, "rev-parse", "HEAD").toString().trim();
  git(source, "clone", "--quiet", "--shared", ".", clone);
  write(source, "new-helper.mjs", "export const value = 'staged';\n");
  git(source, "add", "new-helper.mjs");
  write(source, "new-helper.mjs", "export const value = 'unstaged';\n");
  const snapshot = commitIndexSnapshot(source, clone);
  assert.equal(snapshot.changed, true);
  assert.notEqual(snapshot.head, original);
  assert.equal(
    git(clone, "rev-parse", "master").toString().trim(),
    snapshot.head,
  );
  assert.equal(
    git(
      clone,
      "rev-parse",
      historicalMasterRef("master~1", original, snapshot.changed),
    )
      .toString()
      .trim(),
    previous,
  );
  assert.equal(
    git(clone, "show", `${snapshot.head}:new-helper.mjs`).toString(),
    "export const value = 'staged';\n",
  );
  assert.equal(git(clone, "status", "--porcelain").toString(), "");
});

test("색인의 바이너리와 심볼릭 링크 내용을 복제한다", () => {
  const parent = mkdtempSync(join(tmpdir(), "h57-bytes-"));
  const source = join(parent, "source");
  const clone = join(parent, "clone");
  mkdirSync(source);
  git(source, "init", "--quiet", "-b", "master");
  write(source, "root.txt", "base\n");
  git(source, "add", ".");
  git(source, "commit", "--quiet", "-m", "base");
  git(source, "clone", "--quiet", "--shared", ".", clone);
  writeFileSync(join(source, "data.bin"), Buffer.from([0, 255, 47, 127]));
  symlinkSync("root.txt", join(source, "link"));
  git(source, "add", "data.bin", "link");
  writeFileSync(join(source, "data.bin"), Buffer.from([0, 17, 18]));
  applyIndexToClone(source, clone);
  assert.deepEqual(
    readFileSync(join(clone, "data.bin")),
    Buffer.from([0, 255, 47, 127]),
  );
  assert.equal(readlinkSync(join(clone, "link")), "root.txt");
});

test("복제본의 기준 트리가 다르면 빈 패치라도 실패한다", () => {
  const parent = mkdtempSync(join(tmpdir(), "h57-stale-"));
  const source = join(parent, "source");
  const clone = join(parent, "clone");
  mkdirSync(source);
  git(source, "init", "--quiet", "-b", "master");
  write(source, "file.txt", "old\n");
  git(source, "add", ".");
  git(source, "commit", "--quiet", "-m", "old");
  git(source, "clone", "--quiet", "--shared", ".", clone);
  write(source, "file.txt", "new\n");
  git(source, "add", ".");
  git(source, "commit", "--quiet", "-m", "new");
  assert.throws(() => applyIndexToClone(source, clone), /색인 트리/u);
});
