import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, test } from "node:test";
import {
  LOCK_FILE,
  SIGNERS_FILE,
  parseLock,
  readTrust,
} from "../signatures/verify.mjs";
import {
  EXTRA_TESTS,
  MEMBER_MANIFEST,
  PACKAGE,
  PROTECTED,
  SCRIPT_TEST,
  commit,
  createRepository,
  edit,
  gitIn,
  makeKey,
  read,
  removeRepository,
  repoRoot,
  run,
  scratch,
  signersFor,
  verifyIn,
  write,
} from "./signature-repo.mjs";

const keys = {};
const created = [];

before(() => {
  const directory = scratch("signature-keys-");
  created.push(directory);
  keys.user = makeKey(directory, "user");
  keys.agent = makeKey(directory, "agent");
});

after(() => created.forEach(removeRepository));

const repository = () => {
  const made = createRepository();
  created.push(made.directory);
  return made;
};

const failuresOf = (outcome) => outcome.failures;

const lockedAt = (made, head, extra = {}) =>
  verifyIn({
    directory: made.directory,
    base: made.base,
    head,
    lock: made.base,
    signers: signersFor(keys.user),
    ...extra,
  });

test("the repository ships an empty signer list and an empty lock point", () => {
  assert.equal(readFileSync(join(repoRoot, SIGNERS_FILE), "utf8"), "");
  assert.equal(readFileSync(join(repoRoot, LOCK_FILE), "utf8"), "");
  assert.deepEqual(readTrust(repoRoot).lock, { state: "empty" });
});

test("the lock point is empty or exactly one full commit id line", () => {
  assert.deepEqual(parseLock(""), { state: "empty" });
  assert.deepEqual(parseLock(`${"a".repeat(40)}\n`), {
    state: "locked",
    commit: "a".repeat(40),
  });
  [
    undefined,
    "\n",
    "a".repeat(40),
    `${"a".repeat(39)}\n`,
    `${"A".repeat(40)}\n`,
    "master\n",
  ].forEach((text) =>
    assert.ok(parseLock(text).problem !== undefined, String(text)),
  );
});

test("without a lock point the check says it is inactive and passes unsigned protected changes", () => {
  const made = repository();
  write(made.directory, PROTECTED, "check:\n    false\n");
  const head = commit(made.directory, "unsigned protected");
  const outcome = verifyIn({
    directory: made.directory,
    base: made.base,
    head,
    signers: "",
  });
  assert.deepEqual(outcome.failures, []);
  assert.match(outcome.lines.join("\n"), /서명 검사 비활성: 잠금 시점 없음/u);
});

test("an unsigned commit changing a protected path fails", () => {
  const made = repository();
  write(made.directory, PROTECTED, "check:\n    false\n");
  const head = commit(made.directory, "unsigned protected");
  const failures = failuresOf(lockedAt(made, head));
  assert.equal(failures.length, 1);
  assert.match(failures[0], new RegExp(`^${head} .*보호 경로 justfile`, "u"));
});

test("a commit signed by an allowed signer may change a protected path", () => {
  const made = repository();
  write(made.directory, PROTECTED, "check:\n    false\n");
  const head = commit(made.directory, "signed protected", keys.user);
  const outcome = lockedAt(made, head);
  assert.deepEqual(outcome.failures, []);
  assert.match(outcome.lines.join("\n"), new RegExp(`서명 확인: ${head}`, "u"));
});

test("a signature from a key outside the base signer list fails", () => {
  const made = repository();
  write(made.directory, PROTECTED, "check:\n    false\n");
  const head = commit(made.directory, "agent signed", keys.agent);
  assert.equal(failuresOf(lockedAt(made, head)).length, 1);
  assert.equal(
    failuresOf(lockedAt(made, head, { signers: "" })).length,
    1,
    "an empty signer list accepts nobody",
  );
});

test("adding the agent key and widening the allowlist in the pushed commits changes nothing", () => {
  const made = repository();
  write(made.directory, SIGNERS_FILE, signersFor(keys.agent));
  write(made.directory, "harness/agent-writable.txt", "**\n");
  write(made.directory, PROTECTED, "check:\n    false\n");
  const head = commit(made.directory, "self approval", keys.agent);
  const failures = failuresOf(lockedAt(made, head));
  assert.equal(failures.length, 1);
  assert.match(failures[0], /보호 경로 harness\/agent-writable\.txt/u);
  assert.match(failures[0], /보호 경로 harness\/allowed_signers/u);
});

test("a signed commit whose content was changed after signing fails", () => {
  const made = repository();
  write(made.directory, PROTECTED, "check:\n    false\n");
  const signed = commit(made.directory, "signed protected", keys.user);
  write(made.directory, PROTECTED, "check:\n    rm -rf target\n");
  gitIn(made.directory, ["add", "--all"]);
  const tree = gitIn(made.directory, ["write-tree"]);
  const original = gitIn(made.directory, ["cat-file", "commit", signed]);
  const forged = original.replace(/^tree [0-9a-f]{40}$/mu, `tree ${tree}`);
  assert.notEqual(forged, original);
  write(made.directory, "forged-commit.txt", `${forged}\n`);
  const head = gitIn(made.directory, [
    "hash-object",
    "-t",
    "commit",
    "-w",
    "forged-commit.txt",
  ]);
  assert.match(
    gitIn(made.directory, ["cat-file", "commit", head]),
    /^gpgsig /mu,
  );
  assert.equal(failuresOf(lockedAt(made, head)).length, 1);
  assert.deepEqual(failuresOf(lockedAt(made, signed)), []);
});

test("an unsigned protected change reverted later in the same push still fails", () => {
  const made = repository();
  const original = read(made.directory, PROTECTED);
  write(made.directory, PROTECTED, "check:\n    false\n");
  const changed = commit(made.directory, "unsigned protected");
  write(made.directory, PROTECTED, original);
  const head = commit(made.directory, "revert");
  const failures = failuresOf(lockedAt(made, head));
  assert.equal(failures.length, 2);
  assert.match(failures[0], new RegExp(`^${changed} `, "u"));
  assert.equal(
    gitIn(made.directory, ["diff", made.base, head, "--name-only"]),
    "",
  );
});

test("a merge needs a signature only for content that matches neither parent", () => {
  const made = repository();
  gitIn(made.directory, ["checkout", "--quiet", "-b", "side"]);
  write(made.directory, PROTECTED, "check:\n    false\n");
  commit(made.directory, "signed side", keys.user);
  gitIn(made.directory, ["checkout", "--quiet", "master"]);
  write(made.directory, "docs/notes.md", "notes\n");
  commit(made.directory, "docs");
  gitIn(made.directory, ["merge", "--quiet", "--no-ff", "--no-edit", "side"]);
  const merged = gitIn(made.directory, ["rev-parse", "HEAD"]);
  assert.deepEqual(failuresOf(lockedAt(made, merged)), []);
  write(made.directory, PROTECTED, "check:\n    rm -rf /\n");
  gitIn(made.directory, ["add", "--all"]);
  gitIn(made.directory, ["commit", "--quiet", "--amend", "--no-edit"]);
  const evil = gitIn(made.directory, ["rev-parse", "HEAD"]);
  assert.equal(
    gitIn(made.directory, ["rev-list", "--parents", "-n", "1", evil]).split(" ")
      .length,
    3,
  );
  const failures = failuresOf(lockedAt(made, evil));
  assert.equal(failures.length, 1);
  assert.match(failures[0], new RegExp(`^${evil} .*보호 경로 justfile`, "u"));
});

test("an unsigned side commit is caught even when a signed merge brings it in", () => {
  const made = repository();
  gitIn(made.directory, ["checkout", "--quiet", "-b", "side"]);
  write(made.directory, PROTECTED, "check:\n    false\n");
  const side = commit(made.directory, "unsigned side");
  gitIn(made.directory, ["checkout", "--quiet", "master"]);
  write(made.directory, "docs/notes.md", "notes\n");
  commit(made.directory, "docs");
  gitIn(made.directory, [
    "-c",
    `user.signingkey=${keys.user.file}`,
    "merge",
    "--quiet",
    "--no-ff",
    "--no-edit",
    "-S",
    "side",
  ]);
  const merged = gitIn(made.directory, ["rev-parse", "HEAD"]);
  const failures = failuresOf(lockedAt(made, merged));
  assert.equal(failures.length, 1);
  assert.match(failures[0], new RegExp(`^${side} `, "u"));
});

test("fast-forwarded commits are checked one by one", () => {
  const made = repository();
  write(made.directory, "docs/a.md", "a\n");
  commit(made.directory, "docs a");
  write(made.directory, PROTECTED, "check:\n    false\n");
  const middle = commit(made.directory, "unsigned protected");
  write(made.directory, "docs/b.md", "b\n");
  const head = commit(made.directory, "docs b");
  const outcome = lockedAt(made, head);
  assert.match(outcome.lines[0], /커밋 3개/u);
  assert.equal(outcome.failures.length, 1);
  assert.match(outcome.failures[0], new RegExp(`^${middle} `, "u"));
});

test("dependency tables of writable manifests may change unsigned, other tables may not", () => {
  const made = repository();
  edit(
    made.directory,
    MEMBER_MANIFEST,
    "publish = false\n",
    'publish = false\n\n[dependencies]\nserde = "=1.0.229"\n',
  );
  const cargo = commit(made.directory, "cargo dependency");
  const packageText = JSON.parse(read(made.directory, PACKAGE));
  write(
    made.directory,
    PACKAGE,
    `${JSON.stringify({ ...packageText, devDependencies: { vitest: "5.1.0" } }, undefined, 2)}\n`,
  );
  const npm = commit(made.directory, "package dependency");
  assert.deepEqual(failuresOf(lockedAt(made, npm)), []);
  edit(
    made.directory,
    MEMBER_MANIFEST,
    'version = "0.1.0"',
    'version = "0.2.0"',
  );
  const version = commit(made.directory, "cargo version");
  write(
    made.directory,
    PACKAGE,
    `${JSON.stringify({ ...packageText, scripts: { build: "true" } }, undefined, 2)}\n`,
  );
  const scripts = commit(made.directory, "package scripts");
  const failures = failuresOf(lockedAt(made, scripts));
  assert.equal(failures.length, 2);
  assert.match(
    failures[0],
    new RegExp(`^${version} .*의존성 절 밖 변경 ${MEMBER_MANIFEST}`, "u"),
  );
  assert.match(
    failures[1],
    new RegExp(`^${scripts} .*의존성 절 밖 변경 ${PACKAGE}`, "u"),
  );
  assert.ok(cargo !== version);
});

test("changing an existing Rust test or test script needs a signature, a new test does not", () => {
  const made = repository();
  edit(
    made.directory,
    EXTRA_TESTS,
    "#[test]\nfn adds_zero() {",
    '#[test]\nfn adds_one() {\n    assert_eq!(super::add(1, 0), 1, "one");\n}\n\n#[test]\nfn adds_zero() {',
  );
  write(
    made.directory,
    "crates/sample/src/new.test.mjs",
    'test("new", () => {});\n',
  );
  const added = commit(made.directory, "new tests");
  assert.deepEqual(failuresOf(lockedAt(made, added)), []);
  edit(
    made.directory,
    EXTRA_TESTS,
    'assert_eq!(super::add(4, 0), 4, "zero");',
    'assert!(true, "zero");',
  );
  const weakened = commit(made.directory, "weaken rust test");
  write(made.directory, SCRIPT_TEST, "");
  const emptied = commit(made.directory, "empty test script");
  edit(
    made.directory,
    EXTRA_TESTS,
    'assert_eq!(super::add(1, 0), 1, "one");',
    'assert!(true, "one");',
  );
  const own = commit(made.directory, "change own new test");
  const failures = failuresOf(lockedAt(made, own));
  assert.equal(failures.length, 2);
  assert.match(
    failures[0],
    new RegExp(`^${weakened} .*기존 테스트 변경 .*adds_zero`, "u"),
  );
  assert.match(
    failures[1],
    new RegExp(`^${emptied} .*기존 테스트 변경 ${SCRIPT_TEST}`, "u"),
  );
});

test("commits before the lock point are not checked", () => {
  const made = repository();
  write(made.directory, PROTECTED, "check:\n    false\n");
  const old = commit(made.directory, "unsigned before lock");
  write(made.directory, "docs/a.md", "a\n");
  const head = commit(made.directory, "docs");
  assert.equal(failuresOf(lockedAt(made, head)).length, 1);
  const outcome = lockedAt(made, head, { lock: old });
  assert.deepEqual(outcome.failures, []);
  assert.match(outcome.lines[0], /커밋 1개/u);
});

test("recording the lock point needs a signature from the recorded signer list", () => {
  const made = repository();
  write(made.directory, SIGNERS_FILE, signersFor(keys.user));
  write(made.directory, LOCK_FILE, `${made.base}\n`);
  const unsigned = commit(made.directory, "record lock unsigned");
  const outcome = verifyIn({
    directory: made.directory,
    base: made.base,
    head: unsigned,
    signers: "",
  });
  assert.equal(outcome.failures.length, 1);
  assert.match(outcome.failures[0], /잠금 시점을 기록하는 커밋은/u);
  gitIn(made.directory, ["reset", "--quiet", "--hard", made.base]);
  write(made.directory, SIGNERS_FILE, signersFor(keys.user));
  write(made.directory, LOCK_FILE, `${made.base}\n`);
  const signed = commit(made.directory, "record lock signed", keys.user);
  const accepted = verifyIn({
    directory: made.directory,
    base: made.base,
    head: signed,
    signers: "",
  });
  assert.deepEqual(accepted.failures, []);
  assert.match(
    accepted.lines.join("\n"),
    new RegExp(`잠금 시점 기록: ${signed}`, "u"),
  );
  write(made.directory, LOCK_FILE, "master\n");
  const malformed = commit(made.directory, "bad lock", keys.user);
  assert.equal(
    verifyIn({
      directory: made.directory,
      base: made.base,
      head: malformed,
      signers: "",
    }).failures.length,
    1,
  );
});

test("the command line reads its trust data from its own checkout", () => {
  const made = repository();
  const outcome = run(repoRoot, "node", [
    "harness/ci/signatures/verify.mjs",
    made.directory,
    made.base,
    made.base,
  ]);
  assert.match(outcome, /서명 검사 비활성: 잠금 시점 없음/u);
  assert.match(outcome, /서명 검사: 위반 0건/u);
});
