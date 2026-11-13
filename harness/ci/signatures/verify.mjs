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
  fetchedOwnerKeys,
  fileOwnerKeys,
  missingOwnerKeys,
} from "./owner-keys.mjs";
import {
  configNameMatches,
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

export const RULES_FILE = "harness/claude-hooks/rules.json";

export const OWNER_FILE = "harness/ci/signatures/owner.txt";

const configNamesOf = (text) => {
  try {
    const names = JSON.parse(text).paths.configFiles.names;
    return Array.isArray(names) ? names : undefined;
  } catch {
    return undefined;
  }
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
    configNames: configNamesOf(read(RULES_FILE) ?? ""),
    owner: (read(OWNER_FILE) ?? "").trim(),
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

const manifestReason = (context, parent, commit, path) => {
  const { xtask, repo, base } = context;
  const kind = manifestKind(path);
  if (kind === "cargo") {
    const run = xtask(["manifest-changes", repo, base, parent, commit, path]);
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
    const kind = manifestKind(path);
    const config =
      kind === undefined && configNameMatches(context.configNames, path)
        ? [`설정 파일 이름 ${path}`]
        : [];
    const testScript =
      path.toLowerCase().endsWith(TEST_SCRIPT_SUFFIX) &&
      blob(repo, parent, path) !== undefined
        ? [`기존 테스트 변경 ${path}`]
        : [];
    return [
      ...manifestReason(context, parent, commit, path),
      ...config,
      ...testScript,
    ];
  });
  const rust = paths.some((path) => path.endsWith(RUST_SUFFIX))
    ? testChangeReasons(xtask, repo, base, parent, commit)
    : [];
  return [...new Set([...pathReasons, ...rust])].sort();
};

const parentsOf = (repo, commit) =>
  gitLines(repo, ["rev-list", "--parents", "-n", "1", commit])[0]
    .split(" ")
    .slice(1);

const mergeBases = (repo, left, right) => {
  const run = git(repo, ["merge-base", "--all", left, right]);
  return run.status === 0
    ? run.stdout.split("\n").filter((line) => line !== "")
    : [];
};

const isAncestor = (repo, ancestor, descendant) =>
  git(repo, ["merge-base", "--is-ancestor", ancestor, descendant]).status === 0;

const sideIntroduced = (context, from, to, reason) => {
  const bases = mergeBases(context.repo, from, to);
  return (
    bases.length === 1 && changeReasons(context, bases[0], to).includes(reason)
  );
};

export const requiredReasons = (context, commit) => {
  const parents = parentsOf(context.repo, commit);
  if (parents.length <= 1) {
    return changeReasons(context, parents[0] ?? EMPTY_TREE, commit);
  }
  const byParent = new Map(
    parents.map((parent) => [parent, changeReasons(context, parent, commit)]),
  );
  const all = [...new Set([...byParent.values()].flat())].sort();
  return all.filter(
    (reason) =>
      !parents.some(
        (source) =>
          !byParent.get(source).includes(reason) &&
          parents
            .filter(
              (other) =>
                other !== source && byParent.get(other).includes(reason),
            )
            .every((other) => sideIntroduced(context, other, source, reason)),
      ),
  );
};

const subject = (repo, commit) =>
  gitLines(repo, ["log", "-1", "--format=%s", commit])[0] ?? "";

const lockRecordings = (repo, base, head) =>
  gitLines(repo, ["rev-list", "--reverse", "--topo-order", head, `^${base}`])
    .map((commit) => ({
      commit,
      lock: parseLock(blob(repo, commit, LOCK_FILE) ?? ""),
    }))
    .filter((entry) => entry.lock.state !== "empty");

export const SIGNER_LINE =
  /^[^\s"]+ namespaces="git" (?:ssh-ed25519|sk-ssh-ed25519@openssh\.com|ecdsa-sha2-nistp256|sk-ecdsa-sha2-nistp256@openssh\.com) [A-Za-z0-9+/]+={0,2}(?: [^\s"]+)?$/u;

export const signerProblems = (text) =>
  text === ""
    ? []
    : !text.endsWith("\n")
      ? [`${SIGNERS_FILE}: 마지막 줄이 줄바꿈으로 끝나지 않는다`]
      : text
          .slice(0, -1)
          .split("\n")
          .flatMap((line, index) =>
            SIGNER_LINE.test(line)
              ? []
              : [
                  `${SIGNERS_FILE} ${index + 1}번째 줄은 '<이름> namespaces="git" <키 종류> <키>' 형식만 받는다(만료 옵션·다른 namespace·주석 금지)`,
                ],
          );

export const OWNER_KEY =
  /^(?:ssh-ed25519|sk-ssh-ed25519@openssh\.com|ecdsa-sha2-nistp256|sk-ecdsa-sha2-nistp256@openssh\.com) [A-Za-z0-9+/]+={0,2}$/u;

export const ownerSigners = (ownerKeys) =>
  ownerKeys
    .filter((key) => OWNER_KEY.test(key))
    .map((key) => `owner namespaces="git" ${key}\n`)
    .join("");

const recordingProblems = (repo, recording, ownerKeys) => {
  if (recording.lock.problem !== undefined) {
    return [`${recording.commit}: ${recording.lock.problem}`];
  }
  const own = blob(repo, recording.commit, SIGNERS_FILE) ?? "";
  const ownProblems = signerProblems(own).map(
    (problem) => `${recording.commit}: ${problem}`,
  );
  return [
    ...ownProblems,
    ...(ownProblems.length === 0 && signedBy(repo, own, recording.commit)
      ? []
      : [
          `${recording.commit}: 잠금 시점을 기록하는 커밋은 그 커밋의 ${SIGNERS_FILE}에 있는 키로 서명해야 한다`,
        ]),
    ...(() => {
      const owner = ownerKeys();
      return owner.problem !== undefined
        ? [`${recording.commit}: ${owner.problem}`]
        : signedBy(repo, ownerSigners(owner.keys), recording.commit)
          ? []
          : [
              `${recording.commit}: 잠금 시점을 기록하는 커밋의 서명 키가 저장소 소유자의 GitHub SSH 서명 키에 없다`,
            ];
    })(),
    ...(parentsOf(repo, recording.commit)[0] === recording.lock.commit
      ? []
      : [
          `${recording.commit}: 처음 기록하는 잠금 시점은 이 커밋의 첫 부모여야 한다(기록: ${recording.lock.commit})`,
        ]),
  ];
};

const lockChangeProblems = (repo, base, lock, commit) => {
  const text = blob(repo, commit, LOCK_FILE);
  if (text === blob(repo, base, LOCK_FILE)) {
    return [];
  }
  const recorded = parseLock(text ?? "");
  if (recorded.state !== "locked") {
    return [
      `${commit}: 기록된 잠금 시점을 비우거나 지울 수 없다(서명이 있어도 실패)`,
    ];
  }
  if (recorded.commit === lock) {
    return [];
  }
  return isAncestor(repo, lock, recorded.commit) &&
    isAncestor(repo, recorded.commit, commit)
    ? []
    : [
        `${commit}: 잠금 시점은 기존 잠금 시점의 자손이고 이 커밋의 조상인 커밋으로만 옮길 수 있다`,
      ];
};

const memoized = (provide) => {
  const cache = [];
  return () => {
    if (cache.length === 0) {
      cache.push(provide());
    }
    return cache[0];
  };
};

export const BASE_KINDS = ["verified", "unverified"];

export const verify = ({
  repo,
  base,
  head,
  trust,
  xtask,
  ownerKeys,
  baseKind,
}) => {
  const lines = [];
  const failures = [];
  if (!BASE_KINDS.includes(baseKind)) {
    return {
      lines,
      failures: [
        `기준 종류는 ${BASE_KINDS.join(" 또는 ")}여야 한다: ${baseKind}`,
      ],
    };
  }
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
  if (!Array.isArray(trust.configNames)) {
    return {
      lines,
      failures: [`${RULES_FILE}의 설정 파일 이름 목록을 읽지 못했다`],
    };
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
        recordingProblems(
          repo,
          recording,
          memoized(ownerKeys ?? missingOwnerKeys),
        ),
      ),
    };
  }
  if (baseKind !== "verified") {
    return {
      lines,
      failures: [
        `기준 ${base}은 서명 검사가 통과한 master 커밋이 아닌데 잠금 시점이 기록돼 있다`,
      ],
    };
  }
  const signers = signerProblems(trust.signers);
  if (signers.length > 0) {
    return { lines, failures: signers };
  }
  const lock = trust.lock.commit;
  if (git(repo, ["cat-file", "-e", `${lock}^{commit}`]).status !== 0) {
    return { lines, failures: [`잠금 시점 커밋 ${lock}을 찾지 못했다`] };
  }
  if (!isAncestor(repo, lock, head)) {
    return {
      lines,
      failures: [`대상 ${head}이 잠금 시점 ${lock}의 자손이 아니다`],
    };
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
  const context = {
    repo,
    base,
    allowlist: trust.allowlist,
    configNames: trust.configNames,
    xtask,
  };
  commits.forEach((commit) => {
    failures.push(...lockChangeProblems(repo, base, lock, commit));
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

export const parseCommandLine = (args) => {
  const [repo, base, head, ...rest] = args;
  if (repo === undefined || base === undefined || head === undefined) {
    return { problem: "인자 부족" };
  }
  const options = { repo, base, head };
  for (let index = 0; index < rest.length; index += 2) {
    const value = rest[index + 1];
    if (value === undefined) {
      return { problem: `${rest[index]}의 값이 없다` };
    }
    if (rest[index] === "--owner-keys") {
      options.ownerKeysFile = value;
    } else if (rest[index] === "--base-kind" && BASE_KINDS.includes(value)) {
      options.baseKind = value;
    } else {
      return { problem: `알 수 없는 인자 ${rest[index]} ${value}` };
    }
  }
  return options.baseKind === undefined
    ? { problem: "--base-kind verified|unverified가 필요하다" }
    : options;
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const options = parseCommandLine(process.argv.slice(2));
  if (options.problem !== undefined) {
    process.stderr.write(
      "사용법: node harness/ci/signatures/verify.mjs <저장소> <범위 시작 커밋> <대상 커밋> --base-kind verified|unverified [--owner-keys <파일>]\n",
    );
    process.exitCode = 2;
  } else {
    const trust = readTrust(toolsRoot);
    const outcome = verify({
      repo: resolve(options.repo),
      base: options.base,
      head: options.head,
      trust,
      xtask: xtaskRunner(toolsRoot),
      ownerKeys:
        options.ownerKeysFile === undefined
          ? fetchedOwnerKeys(trust.owner, process.env.GH_TOKEN ?? "")
          : fileOwnerKeys(options.ownerKeysFile),
      baseKind: options.baseKind,
    });
    outcome.lines.forEach((line) => process.stdout.write(`${line}\n`));
    outcome.failures.forEach((failure) =>
      process.stdout.write(`FAIL ${failure}\n`),
    );
    process.stdout.write(`서명 검사: 위반 ${outcome.failures.length}건\n`);
    process.exitCode = outcome.failures.length === 0 ? 0 : 1;
  }
}
