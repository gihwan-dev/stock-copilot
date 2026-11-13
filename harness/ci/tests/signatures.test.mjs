import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { after, before, test } from "node:test";
import {
  LOCK_FILE,
  SIGNERS_FILE,
  parseLock,
  readTrust,
  signedBy,
  signerProblems,
  verify,
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
  ownerKeyOf,
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

const recordLock = (made, key, signerKeys = [keys.user]) => {
  write(made.directory, SIGNERS_FILE, signersFor(...signerKeys));
  write(made.directory, LOCK_FILE, `${made.base}\n`);
  return commit(made.directory, "record lock", key);
};

const inactive = (made, head, ownerKeys) =>
  verifyIn({
    directory: made.directory,
    base: made.base,
    head,
    signers: "",
    ownerKeys,
  });

test("recording the lock point needs a signature from the recorded list and from the owner's GitHub keys", () => {
  const owner = [ownerKeyOf(keys.user)];
  const made = repository();
  const signed = recordLock(made, keys.user);
  const accepted = inactive(made, signed, owner);
  assert.deepEqual(accepted.failures, []);
  assert.match(
    accepted.lines.join("\n"),
    new RegExp(`잠금 시점 기록: ${signed}`, "u"),
  );
  assert.match(
    inactive(made, signed, undefined).failures.join("\n"),
    /소유자의 SSH 서명 키 목록이 없어/u,
  );
  assert.match(
    inactive(made, signed, [ownerKeyOf(keys.agent)]).failures.join("\n"),
    /소유자의 GitHub SSH 서명 키에 없다/u,
  );
  write(made.directory, LOCK_FILE, "master\n");
  const malformed = commit(made.directory, "bad lock", keys.user);
  assert.equal(inactive(made, malformed, owner).failures.length, 1);
});

test("an agent key placed in the recorded signer list does not pass the owner check", () => {
  const made = repository();
  const agentRecorded = recordLock(made, keys.agent, [keys.agent]);
  const failures = inactive(made, agentRecorded, [
    ownerKeyOf(keys.user),
  ]).failures;
  assert.equal(failures.length, 1);
  assert.match(failures[0], /소유자의 GitHub SSH 서명 키에 없다/u);
  const other = repository();
  const unsignedRecord = recordLock(other, undefined);
  assert.match(
    inactive(other, unsignedRecord, [ownerKeyOf(keys.user)]).failures.join(
      "\n",
    ),
    /그 커밋의 harness\/allowed_signers에 있는 키로 서명해야 한다/u,
  );
});

test("signer lists accept only plain git-namespace keys without expiry options", () => {
  assert.deepEqual(signerProblems(""), []);
  assert.deepEqual(signerProblems(signersFor(keys.user, keys.agent)), []);
  const [type, body] = keys.user.public.split(" ");
  [
    `a valid-before="20200101" namespaces="git" ${type} ${body}\n`,
    `a namespaces="git",valid-after="20990101" ${type} ${body}\n`,
    `a namespaces="file" ${type} ${body}\n`,
    `a ${type} ${body}\n`,
    `# comment\n`,
    `a namespaces="git" ssh-rsa ${body}\n`,
    `a namespaces="git" ${type} ${body}`,
  ].forEach((text) =>
    assert.equal(signerProblems(text).length, 1, JSON.stringify(text)),
  );
  const made = repository();
  write(made.directory, PROTECTED, "check:\n    false\n");
  const head = commit(made.directory, "signed protected", keys.user);
  assert.match(
    failuresOf(
      lockedAt(made, head, {
        signers: `a valid-before="20200101" namespaces="git" ${type} ${body}\n`,
      }),
    ).join("\n"),
    /만료 옵션/u,
  );
  assert.equal(
    signedBy(made.directory, `a namespaces="file" ${type} ${body}\n`, head),
    false,
    "a key outside the git namespace does not verify a commit",
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

const lockedRepository = () => {
  const made = repository();
  write(made.directory, LOCK_FILE, `${made.base}\n`);
  const locked = commit(made.directory, "lock", keys.user);
  return { directory: made.directory, base: locked, lock: made.base };
};

const checkLocked = (made, head) =>
  verifyIn({
    directory: made.directory,
    base: made.base,
    head,
    lock: made.lock,
    signers: signersFor(keys.user),
  }).failures;

const forgeMerge = (made, treeOf, parents) => {
  const tree = gitIn(made.directory, ["rev-parse", `${treeOf}^{tree}`]);
  return gitIn(made.directory, [
    "commit-tree",
    tree,
    ...parents.flatMap((parent) => ["-p", parent]),
    "-m",
    "forged merge",
  ]);
};

const restoredTree = (made, from, paths) => {
  gitIn(made.directory, ["checkout", "--quiet", "--detach", made.base]);
  paths.forEach((path) =>
    gitIn(made.directory, ["checkout", from, "--", path]),
  );
  write(made.directory, "docs/new.md", "new\n");
  return commit(made.directory, "restored tree source");
};

test("a merge with an ancestor parent cannot restore old protected files or empty the lock point", () => {
  const made = lockedRepository();
  write(made.directory, PROTECTED, "check:\n    false\n");
  const c2 = commit(made.directory, "signed protected", keys.user);
  const source = restoredTree(made, made.lock, [PROTECTED, LOCK_FILE]);
  assert.equal(read(made.directory, LOCK_FILE), "");
  [
    [c2, made.lock],
    [made.lock, c2],
    [c2, made.lock, made.base],
  ].forEach((parents) => {
    const forged = forgeMerge(made, source, parents);
    const failures = checkLocked(made, forged).join("\n");
    assert.match(
      failures,
      new RegExp(`${forged}: 기록된 잠금 시점을 비우거나`, "u"),
      parents.join(" "),
    );
    assert.match(
      failures,
      new RegExp(`${forged} .*보호 경로 justfile`, "u"),
      parents.join(" "),
    );
  });
});

test("a side commit grown from an old ancestor cannot carry old protected content into a merge", () => {
  const made = lockedRepository();
  write(made.directory, PROTECTED, "check:\n    false\n");
  const c2 = commit(made.directory, "signed protected", keys.user);
  gitIn(made.directory, ["checkout", "--quiet", "--detach", made.base]);
  write(made.directory, "docs/side.md", "side\n");
  const side = commit(made.directory, "side from the old base");
  gitIn(made.directory, ["checkout", "--quiet", c2]);
  gitIn(made.directory, [
    "merge",
    "--quiet",
    "--no-ff",
    "--no-edit",
    "-s",
    "ours",
    side,
  ]);
  gitIn(made.directory, ["checkout", side, "--", PROTECTED]);
  gitIn(made.directory, ["commit", "--quiet", "--amend", "--no-edit"]);
  const merged = gitIn(made.directory, ["rev-parse", "HEAD"]);
  const failures = checkLocked(made, merged);
  assert.equal(failures.length, 1);
  assert.match(failures[0], new RegExp(`^${merged} .*보호 경로 justfile`, "u"));
});

test("merging the base into a branch takes the base's protected changes without a signature", () => {
  const made = lockedRepository();
  gitIn(made.directory, ["checkout", "--quiet", "-b", "branch"]);
  write(made.directory, "docs/branch.md", "branch\n");
  commit(made.directory, "branch work");
  gitIn(made.directory, ["checkout", "--quiet", "--detach", made.base]);
  write(made.directory, PROTECTED, "check:\n    false\n");
  const newBase = commit(made.directory, "signed master change", keys.user);
  gitIn(made.directory, ["checkout", "--quiet", "branch"]);
  gitIn(made.directory, ["merge", "--quiet", "--no-ff", "--no-edit", newBase]);
  const merged = gitIn(made.directory, ["rev-parse", "HEAD"]);
  assert.deepEqual(checkLocked({ ...made, base: newBase }, merged), []);
});

test("moving the lock point forward needs a signature, emptying or moving it back always fails", () => {
  const made = lockedRepository();
  write(made.directory, "docs/a.md", "a\n");
  const later = commit(made.directory, "docs");
  write(made.directory, LOCK_FILE, `${later}\n`);
  const forward = commit(made.directory, "move lock", keys.user);
  assert.deepEqual(checkLocked(made, forward), []);
  write(made.directory, LOCK_FILE, `${later}\n`);
  gitIn(made.directory, ["checkout", "--quiet", "--detach", made.base]);
  write(made.directory, LOCK_FILE, `${later}\n`);
  const unsignedForward = commit(made.directory, "move lock unsigned");
  assert.match(
    checkLocked(made, unsignedForward).join("\n"),
    /보호 경로 harness\/ci\/signatures\/lock-point\.txt/u,
  );
  gitIn(made.directory, ["checkout", "--quiet", "--detach", made.base]);
  write(made.directory, LOCK_FILE, "");
  const emptied = commit(made.directory, "empty lock", keys.user);
  assert.match(checkLocked(made, emptied).join("\n"), /비우거나 지울 수 없다/u);
  gitIn(made.directory, ["checkout", "--quiet", "--detach", made.base]);
  gitIn(made.directory, ["rm", "--quiet", LOCK_FILE]);
  const removed = commit(made.directory, "remove lock", keys.user);
  assert.match(checkLocked(made, removed).join("\n"), /비우거나 지울 수 없다/u);
  gitIn(made.directory, ["checkout", "--quiet", "--detach", made.base]);
  const older = gitIn(made.directory, ["rev-parse", `${made.lock}`]);
  write(made.directory, "docs/b.md", "b\n");
  const sideways = commit(made.directory, "sideways");
  gitIn(made.directory, ["checkout", "--quiet", "--detach", made.base]);
  write(made.directory, LOCK_FILE, `${sideways}\n`);
  const notAncestor = commit(made.directory, "lock not an ancestor", keys.user);
  assert.match(
    checkLocked(made, notAncestor).join("\n"),
    /자손이고 이 커밋의 조상인/u,
  );
  assert.ok(older === made.lock);
});

test("a merge whose parents have more than one merge base needs a signature for protected content", () => {
  const made = lockedRepository();
  write(made.directory, "docs/x1.md", "x1\n");
  const x1 = commit(made.directory, "x1");
  gitIn(made.directory, ["checkout", "--quiet", "--detach", made.base]);
  write(made.directory, "docs/x2.md", "x2\n");
  const x2 = commit(made.directory, "x2");
  gitIn(made.directory, ["checkout", "--quiet", "--detach", x1]);
  gitIn(made.directory, ["merge", "--quiet", "--no-ff", "--no-edit", x2]);
  const q1 = gitIn(made.directory, ["rev-parse", "HEAD"]);
  gitIn(made.directory, ["checkout", "--quiet", "--detach", x2]);
  gitIn(made.directory, ["merge", "--quiet", "--no-ff", "--no-edit", x1]);
  write(made.directory, PROTECTED, "check:\n    false\n");
  const signed = commit(made.directory, "signed protected", keys.user);
  assert.equal(
    gitIn(made.directory, ["merge-base", "--all", q1, signed]).split("\n")
      .length,
    2,
  );
  gitIn(made.directory, ["checkout", "--quiet", "--detach", q1]);
  gitIn(made.directory, ["merge", "--quiet", "--no-ff", "--no-edit", signed]);
  const merged = gitIn(made.directory, ["rev-parse", "HEAD"]);
  assert.equal(read(made.directory, PROTECTED), "check:\n    false\n");
  const failures = checkLocked(made, merged);
  assert.equal(failures.length, 1);
  assert.match(failures[0], new RegExp(`^${merged} .*보호 경로 justfile`, "u"));
});

test("the first lock point must be the recording commit's first parent", () => {
  const made = repository();
  write(made.directory, "docs/a.md", "a\n");
  commit(made.directory, "between");
  write(made.directory, SIGNERS_FILE, signersFor(keys.user));
  write(made.directory, LOCK_FILE, `${made.base}\n`);
  const grandparent = commit(made.directory, "record an older lock", keys.user);
  const owner = [ownerKeyOf(keys.user)];
  assert.match(
    inactive(made, grandparent, owner).failures.join("\n"),
    /첫 부모여야 한다/u,
  );
  const parent = gitIn(made.directory, ["rev-parse", "HEAD"]);
  write(made.directory, LOCK_FILE, `${parent}\n`);
  const fresh = commit(made.directory, "record the parent", keys.user);
  assert.match(
    inactive(made, fresh, owner).failures.join("\n"),
    new RegExp(`${grandparent}: 처음 기록하는 잠금 시점은`, "u"),
  );
  const clean = repository();
  write(clean.directory, "docs/a.md", "a\n");
  const tip = commit(clean.directory, "between");
  write(clean.directory, SIGNERS_FILE, signersFor(keys.user));
  write(clean.directory, LOCK_FILE, `${tip}\n`);
  const recorded = commit(clean.directory, "record the parent", keys.user);
  assert.deepEqual(inactive(clean, recorded, owner).failures, []);
});

test("the owner's keys are fetched only when a lock record is in the pushed range", () => {
  const made = repository();
  write(made.directory, PROTECTED, "check:\n    false\n");
  const head = commit(made.directory, "unrelated");
  const calls = [];
  const provider = () => {
    calls.push("called");
    return { keys: [ownerKeyOf(keys.user)] };
  };
  const quiet = verify({
    repo: made.directory,
    base: made.base,
    head,
    trust: readTrust(repoRoot),
    xtask: () => ({ status: 0, stdout: "", stderr: "" }),
    ownerKeys: provider,
  });
  assert.deepEqual(quiet.failures, []);
  assert.deepEqual(calls, []);
  const recorded = recordLock(made, keys.user);
  verify({
    repo: made.directory,
    base: made.base,
    head: recorded,
    trust: readTrust(repoRoot),
    xtask: () => ({ status: 0, stdout: "", stderr: "" }),
    ownerKeys: provider,
  });
  assert.deepEqual(calls, ["called"]);
});

test("a locked check refuses a base that no successful signature run verified", () => {
  const made = lockedRepository();
  write(made.directory, "docs/a.md", "a\n");
  const head = commit(made.directory, "docs");
  assert.match(
    verifyIn({
      directory: made.directory,
      base: made.base,
      head,
      lock: made.lock,
      signers: signersFor(keys.user),
      baseKind: "unverified",
    }).failures.join("\n"),
    /서명 검사가 통과한 master 커밋이 아닌데/u,
  );
});

test("case variants of manifests, config names and protected folders still need a signature", () => {
  const made = lockedRepository();
  write(
    made.directory,
    "fixtures/evil/cargo.toml",
    '[package]\nname = "evil"\nversion = "0.0.0"\nbuild = "build.rs"\n',
  );
  write(made.directory, "fixtures/evil/build.rs", "fn main() {}\n");
  const manifest = commit(made.directory, "lowercase manifest");
  edit(
    made.directory,
    MEMBER_MANIFEST,
    "publish = false\n",
    'publish = false\n\n[dependencies]\nevil = { path = "../../fixtures/evil" }\n',
  );
  const pathDependency = commit(
    made.directory,
    "path dependency outside the members",
  );
  write(made.directory, "fixtures/x/TSConfig.json", "{}\n");
  const config = commit(made.directory, "config name");
  const protectedPaths = [
    "Harness/x.txt",
    ".GitHub/workflows/x.yml",
    "crates/XTask/src/x.rs",
    "docs/HARNESS.md",
  ].map((path) => {
    write(made.directory, "blob.txt", "x\n");
    const object = gitIn(made.directory, ["hash-object", "-w", "blob.txt"]);
    gitIn(made.directory, [
      "update-index",
      "--add",
      "--cacheinfo",
      `100644,${object},${path}`,
    ]);
    gitIn(made.directory, ["commit", "--quiet", "-m", `case ${path}`]);
    return [path, gitIn(made.directory, ["rev-parse", "HEAD"])];
  });
  const failures = checkLocked(made, protectedPaths.at(-1)[1]).join("\n");
  assert.match(
    failures,
    new RegExp(
      `${manifest} .*의존성 절 밖 변경 fixtures/evil/cargo\\.toml`,
      "u",
    ),
  );
  assert.match(
    failures,
    new RegExp(`${pathDependency} .*의존성 절 밖 변경 ${MEMBER_MANIFEST}`, "u"),
  );
  assert.match(
    failures,
    new RegExp(`${config} .*설정 파일 이름 fixtures/x/TSConfig\\.json`, "u"),
  );
  protectedPaths.forEach(([path, sha]) =>
    assert.match(
      failures,
      new RegExp(`${sha} .*보호 경로 ${path.replace(/\./gu, "\\.")}`, "u"),
      path,
    ),
  );
});
