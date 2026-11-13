import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  isWritable,
  parseAllowlist,
} from "../../../.claude/hooks/lib/allowlist.mjs";
import {
  manifestKind,
  packageChangesOutsideDependencies,
} from "./manifests.mjs";

export const LOCK_FILE = "harness/ci/signatures/lock-point.txt";

export const SIGNERS_FILE = "harness/allowed_signers";

export const ALLOWLIST_FILE = "harness/agent-writable.txt";

export const EMPTY_TREE = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";

const TEST_SCRIPT_SUFFIX = ".test.mjs";

const RUST_SUFFIX = ".rs";

const COMMIT = /^[0-9a-f]{40}$/u;

const REDIRECTING = [
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_INDEX_FILE",
  "GIT_OBJECT_DIRECTORY",
  "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  "GIT_COMMON_DIR",
  "GIT_SHALLOW_FILE",
  "GIT_CONFIG_GLOBAL",
  "GIT_CONFIG_SYSTEM",
  "GIT_CONFIG_COUNT",
  "GIT_CONFIG_PARAMETERS",
  "GIT_DIFF_OPTS",
  "GIT_ATTR_SOURCE",
  "GIT_EXTERNAL_DIFF",
];

const gitEnvironment = () => ({
  ...Object.fromEntries(
    Object.entries(process.env).filter(([name]) => !REDIRECTING.includes(name)),
  ),
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_GRAFT_FILE: "/dev/null",
});

export const git = (repo, args) =>
  spawnSync(
    "git",
    ["-C", repo, "--no-replace-objects", "-c", "core.fsmonitor=false", ...args],
    {
      encoding: "utf8",
      env: gitEnvironment(),
      maxBuffer: 256 * 1024 * 1024,
    },
  );

const gitLines = (repo, args) => {
  const run = git(repo, args);
  if (run.status !== 0) {
    throw new Error(`git ${args.join(" ")}: ${run.stderr.trim()}`);
  }
  return run.stdout.split("\n").filter((line) => line !== "");
};

const gitPaths = (repo, args) => {
  const run = git(repo, args);
  if (run.status !== 0) {
    throw new Error(`git ${args.join(" ")}: ${run.stderr.trim()}`);
  }
  return run.stdout.split("\0").filter((path) => path !== "");
};

const blob = (repo, revision, path) => {
  const run = git(repo, ["cat-file", "blob", `${revision}:${path}`]);
  return run.status === 0 ? run.stdout : undefined;
};

export const parseLock = (text) =>
  text === undefined
    ? { problem: `${LOCK_FILE}가 없다` }
    : text === ""
      ? { state: "empty" }
      : /^[0-9a-f]{40}\n$/u.test(text)
        ? { state: "locked", commit: text.slice(0, 40) }
        : {
            problem: `${LOCK_FILE}는 비어 있거나 40자리 커밋 ID 한 줄이어야 한다`,
          };

export const readTrust = (root) => {
  const read = (path) =>
    existsSync(join(root, path))
      ? readFileSync(join(root, path), "utf8")
      : undefined;
  return {
    lock: parseLock(read(LOCK_FILE)),
    signers: read(SIGNERS_FILE) ?? "",
    allowlist: parseAllowlist(read(ALLOWLIST_FILE) ?? ""),
  };
};

export const xtaskRunner = (tools) => (args) => {
  const run = spawnSync(
    "cargo",
    [
      "run",
      "--locked",
      "--quiet",
      "--manifest-path",
      join(tools, "Cargo.toml"),
      "--package",
      "xtask",
      "--",
      ...args,
    ],
    { cwd: tools, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
  );
  return { status: run.status, stdout: run.stdout, stderr: run.stderr };
};

const withFile = (text, use) => {
  const directory = mkdtempSync(join(tmpdir(), "signers-"));
  try {
    const file = join(directory, "allowed_signers");
    writeFileSync(file, text);
    return use(file);
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
};

export const signedBy = (repo, signers, commit) =>
  signers.trim() !== "" &&
  withFile(
    signers,
    (file) =>
      git(repo, [
        "-c",
        "gpg.format=ssh",
        "-c",
        `gpg.ssh.allowedSignersFile=${file}`,
        "verify-commit",
        commit,
      ]).status === 0,
  );

const testChangeReasons = (xtask, repo, base, parent, commit) => {
  const run = xtask(["test-changes", repo, base, parent, commit]);
  if (run.status === 0) {
    return [];
  }
  const lines = run.stderr.split("\n").filter((line) => line !== "");
  const summary = lines.at(-1) ?? "";
  return /^test-changes: 위반 \d+건$/u.test(summary)
    ? lines.slice(0, -1).map((line) => `기존 테스트 변경 ${line}`)
    : [`기존 테스트 판정 실패: ${run.stderr.trim() || run.stdout.trim()}`];
};

const manifestReason = (xtask, repo, parent, commit, path) => {
  const kind = manifestKind(path);
  if (kind === "cargo") {
    const run = xtask(["manifest-changes", repo, parent, commit, path]);
    return run.status === 0 ? [] : [`의존성 절 밖 변경 ${path}`];
  }
  if (kind === "package") {
    return packageChangesOutsideDependencies(
      blob(repo, parent, path),
      blob(repo, commit, path),
    )
      ? [`의존성 절 밖 변경 ${path}`]
      : [];
  }
  return [];
};

export const changeReasons = (context, parent, commit) => {
  const { repo, base, allowlist, xtask } = context;
  const paths = gitPaths(repo, [
    "diff-tree",
    "-r",
    "-z",
    "--no-renames",
    "--no-ext-diff",
    "--no-textconv",
    "--name-only",
    parent,
    commit,
  ]);
  const pathReasons = paths.flatMap((path) => {
    if (!isWritable(allowlist, path)) {
      return [`보호 경로 ${path}`];
    }
    const testScript =
      path.endsWith(TEST_SCRIPT_SUFFIX) &&
      blob(repo, parent, path) !== undefined
        ? [`기존 테스트 변경 ${path}`]
        : [];
    return [
      ...manifestReason(xtask, repo, parent, commit, path),
      ...testScript,
    ];
  });
  const rust = paths.some((path) => path.endsWith(RUST_SUFFIX))
    ? testChangeReasons(xtask, repo, base, parent, commit)
    : [];
  return [...new Set([...pathReasons, ...rust])].sort();
};

export const requiredReasons = (context, commit) => {
  const parents = gitLines(context.repo, [
    "rev-list",
    "--parents",
    "-n",
    "1",
    commit,
  ])[0]
    .split(" ")
    .slice(1);
  const sides = (parents.length === 0 ? [EMPTY_TREE] : parents).map((parent) =>
    changeReasons(context, parent, commit),
  );
  return sides.reduce((common, side) =>
    common.filter((reason) => side.includes(reason)),
  );
};

const subject = (repo, commit) =>
  gitLines(repo, ["log", "-1", "--format=%s", commit])[0] ?? "";

const isAncestor = (repo, ancestor, descendant) =>
  git(repo, ["merge-base", "--is-ancestor", ancestor, descendant]).status === 0;

const lockRecordings = (repo, base, head) =>
  gitLines(repo, ["rev-list", "--reverse", "--topo-order", head, `^${base}`])
    .map((commit) => ({
      commit,
      lock: parseLock(blob(repo, commit, LOCK_FILE) ?? ""),
    }))
    .filter((entry) => entry.lock.state !== "empty");

const recordingProblems = (repo, recording) => {
  if (recording.lock.problem !== undefined) {
    return [`${recording.commit}: ${recording.lock.problem}`];
  }
  const own = blob(repo, recording.commit, SIGNERS_FILE) ?? "";
  return [
    ...(signedBy(repo, own, recording.commit)
      ? []
      : [
          `${recording.commit}: 잠금 시점을 기록하는 커밋은 그 커밋의 ${SIGNERS_FILE}에 있는 키로 서명해야 한다`,
        ]),
    ...(isAncestor(repo, recording.lock.commit, recording.commit)
      ? []
      : [
          `${recording.commit}: 잠금 시점 ${recording.lock.commit}이 이 커밋의 조상이 아니다`,
        ]),
  ];
};

export const verify = ({ repo, base, head, trust, xtask }) => {
  const lines = [];
  const failures = [];
  if (!COMMIT.test(base) || !COMMIT.test(head)) {
    return {
      lines,
      failures: ["기준과 대상은 40자리 커밋 ID여야 한다"],
    };
  }
  if (trust.allowlist.problem !== undefined) {
    return {
      lines,
      failures: [`${ALLOWLIST_FILE}: ${trust.allowlist.problem}`],
    };
  }
  if (trust.lock.problem !== undefined) {
    return { lines, failures: [trust.lock.problem] };
  }
  if (trust.lock.state === "empty") {
    const recordings = lockRecordings(repo, base, head);
    lines.push(
      `서명 검사 비활성: 잠금 시점 없음 (기준 커밋 ${base}의 ${LOCK_FILE}가 비어 있다)`,
    );
    recordings.forEach((recording) =>
      lines.push(
        `잠금 시점 기록: ${recording.commit} → ${recording.lock.commit ?? "잘못된 값"}`,
      ),
    );
    return {
      lines,
      failures: recordings.flatMap((recording) =>
        recordingProblems(repo, recording),
      ),
    };
  }
  const lock = trust.lock.commit;
  if (git(repo, ["cat-file", "-e", `${lock}^{commit}`]).status !== 0) {
    return { lines, failures: [`잠금 시점 커밋 ${lock}을 찾지 못했다`] };
  }
  const commits = gitLines(repo, [
    "rev-list",
    "--reverse",
    "--topo-order",
    head,
    `^${base}`,
    `^${lock}`,
  ]);
  lines.push(
    `서명 검사: 기준 ${base}, 잠금 시점 ${lock}, 대상 ${head}, 커밋 ${commits.length}개`,
  );
  const context = { repo, base, allowlist: trust.allowlist, xtask };
  commits.forEach((commit) => {
    const reasons = requiredReasons(context, commit);
    if (reasons.length === 0) {
      return;
    }
    if (signedBy(repo, trust.signers, commit)) {
      lines.push(`서명 확인: ${commit} ${subject(repo, commit)}`);
      return;
    }
    failures.push(
      `${commit} ${subject(repo, commit)}: 허용된 서명 없이 바꿨다: ${reasons.join(", ")}`,
    );
  });
  return { lines, failures };
};

const toolsRoot = resolve(
  fileURLToPath(import.meta.url),
  "..",
  "..",
  "..",
  "..",
);

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [repo, base, head] = process.argv.slice(2);
  if (repo === undefined || base === undefined || head === undefined) {
    process.stderr.write(
      "사용법: node harness/ci/signatures/verify.mjs <저장소> <기준 커밋> <대상 커밋>\n",
    );
    process.exitCode = 2;
  } else {
    const outcome = verify({
      repo: resolve(repo),
      base,
      head,
      trust: readTrust(toolsRoot),
      xtask: xtaskRunner(toolsRoot),
    });
    outcome.lines.forEach((line) => process.stdout.write(`${line}\n`));
    outcome.failures.forEach((failure) =>
      process.stdout.write(`FAIL ${failure}\n`),
    );
    process.stdout.write(`서명 검사: 위반 ${outcome.failures.length}건\n`);
    process.exitCode = outcome.failures.length === 0 ? 0 : 1;
  }
}
