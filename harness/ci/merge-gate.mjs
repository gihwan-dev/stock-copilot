import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  COMMIT,
  evidenceFor,
  remoteCheck,
  REPOSITORY,
  verifyReceipt,
} from "./remote-check.mjs";
import {
  COMMIT_CELL,
  isDone,
  progressRows,
  reviewCommitRows,
  ROUND_CELL,
} from "./review-records/check.mjs";

const REDIRECTING = new Set([
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
  "GIT_GRAFT_FILE",
]);

const environment = () => ({
  ...Object.fromEntries(
    Object.entries(process.env).filter(([key]) => !REDIRECTING.has(key)),
  ),
  GIT_CONFIG_GLOBAL: "/dev/null",
  GIT_CONFIG_NOSYSTEM: "1",
  GIT_GRAFT_FILE: "/dev/null",
  GIT_TERMINAL_PROMPT: "0",
});

const git = (root, args, indexFile) =>
  spawnSync(
    "git",
    ["--no-replace-objects", "-C", root, "-c", "core.fsmonitor=false", ...args],
    {
      encoding: "utf8",
      env: {
        ...environment(),
        ...(indexFile === undefined ? {} : { GIT_INDEX_FILE: indexFile }),
      },
      maxBuffer: 16 * 1024 * 1024,
    },
  );
const output = (root, args, indexFile) => {
  const result = git(root, args, indexFile);
  if (result.status !== 0) throw new Error(`git ${args[0]} 실패`);
  return result.stdout.trim();
};
const commit = (root, ref) =>
  output(root, ["rev-parse", "--verify", `${ref}^{commit}`]);
const ancestor = (root, older, newer) =>
  git(root, ["merge-base", "--is-ancestor", older, newer]).status === 0;

export const allowedPath = (path) =>
  path.startsWith("docs/") ||
  [
    ".codex/config.toml",
    "harness/claude-hooks/hashes.json",
    "harness/arch-check/cases.toml",
  ].includes(path) ||
  path.startsWith("harness/sandbox/");

export const inspectMerge = (root, branch, indexFile) => {
  try {
    if (
      !/^[A-Za-z0-9][A-Za-z0-9._/-]*$/u.test(branch) ||
      branch === "master" ||
      git(root, ["check-ref-format", "--branch", branch]).status !== 0
    ) {
      return { problem: "작업 브랜치 이름이 올바르지 않다" };
    }
    const master = commit(root, "refs/heads/master");
    const head = commit(root, `refs/heads/${branch}`);
    if (!ancestor(root, master, head))
      return { problem: "브랜치가 현재 master를 포함하지 않는다" };
    if (
      output(root, ["symbolic-ref", "--quiet", "HEAD"]) !==
        "refs/heads/master" ||
      commit(root, "HEAD") !== master
    ) {
      return { problem: "현재 작업 사본이 master 끝이 아니다" };
    }
    const mergeFile = output(root, ["rev-parse", "--git-path", "MERGE_HEAD"]);
    const merge = readFileSync(resolve(root, mergeFile), "utf8");
    if (merge !== `${head}\n`)
      return { problem: "병합 대상이 해당 브랜치 끝과 다르다" };
    if (output(root, ["ls-files", "--unmerged"], indexFile) !== "")
      return { problem: "병합 충돌이 남아 있다" };
    if (
      git(
        root,
        ["diff-files", "--quiet", "--ignore-submodules=none"],
        indexFile,
      ).status !== 0 ||
      output(
        root,
        ["ls-files", "--others", "--exclude-standard"],
        indexFile,
      ) !== ""
    ) {
      return {
        problem: "작업 사본이 인덱스와 다르거나 추적하지 않은 파일이 있다",
      };
    }
    const changed = git(
      root,
      ["diff", "--cached", "--name-only", "-z", "--no-renames", head, "--"],
      indexFile,
    );
    if (changed.status !== 0)
      return { problem: "병합 결과의 경로를 확인하지 못했다" };
    const disallowed = changed.stdout
      .split("\0")
      .filter((path) => path !== "" && !allowedPath(path));
    if (disallowed.length !== 0)
      return { problem: `허용되지 않은 병합 차이: ${disallowed.join(", ")}` };
    return { head, master, tree: output(root, ["write-tree"], indexFile) };
  } catch (error) {
    return {
      problem: error instanceof Error ? error.message : "병합 상태 확인 실패",
    };
  }
};

export const recordedCommits = (root) => {
  const entries = [];
  const directory = join(root, "docs/reviews");
  for (const file of readdirSync(directory).filter((name) =>
    name.endsWith(".md"),
  )) {
    const text = readFileSync(join(directory, file), "utf8");
    const rows = reviewCommitRows(text);
    if (rows === undefined)
      throw new Error(`docs/reviews/${file}: 커밋 표를 읽지 못했다`);
    for (const { row, line } of rows) {
      const refs = [
        COMMIT_CELL.exec(row[1] ?? ""),
        COMMIT_CELL.exec(row[2] ?? ""),
      ];
      if (
        !ROUND_CELL.test(row[0] ?? "") ||
        refs.some((match) => match === null)
      )
        throw new Error(
          `docs/reviews/${file}:${line}: 커밋 표의 회차 또는 SHA가 올바르지 않다`,
        );
      entries.push(
        ...refs.map((match) => ({
          ref: match[1],
          location: `docs/reviews/${file}:${line}`,
        })),
      );
    }
  }
  const progress = readFileSync(join(root, "docs/progress.md"), "utf8");
  for (const row of progressRows(progress).filter(
    (row) => row.progress && isDone(row.state),
  )) {
    const match = /^([a-f0-9]{7,40})(?:$|\s)/u.exec(row.target);
    if (match !== null)
      entries.push({ ref: match[1], location: `docs/progress.md:${row.line}` });
  }
  return [
    ...new Set(
      entries.map(({ ref, location }) => {
        try {
          return commit(root, ref);
        } catch {
          throw new Error(`${location}: 리뷰 커밋 ${ref}를 확인하지 못했다`);
        }
      }),
    ),
  ];
};

export const advertisedTags = (listing) =>
  listing.split("\n").flatMap((line) => {
    const [sha, ref] = line.split("\t");
    return COMMIT.test(sha ?? "") && ref?.startsWith("refs/tags/") ? [sha] : [];
  });

export const missingRemoteObjects = (root, reviewed, head, advertised) => {
  const refs = advertised.flatMap((ref) => {
    if (!COMMIT.test(ref)) return [];
    try {
      return [commit(root, ref)];
    } catch {
      return [];
    }
  });
  return reviewed.filter(
    (sha) =>
      !ancestor(root, sha, head) &&
      !refs.some((ref) => ancestor(root, sha, ref)),
  );
};

const validateRemoteObjects = (root, head) => {
  const result = git(root, [
    "-c",
    "credential.helper=",
    "ls-remote",
    "--tags",
    `https://github.com/${REPOSITORY}.git`,
  ]);
  if (result.status !== 0) throw new Error("원격 참조를 조회하지 못했다");
  const advertised = advertisedTags(result.stdout);
  const missing = missingRemoteObjects(
    root,
    recordedCommits(root),
    head,
    advertised,
  );
  if (missing.length !== 0)
    throw new Error(`원격에서 도달할 수 없는 리뷰 커밋: ${missing.join(", ")}`);
};

const validate = async (root, branch, receiptPath, indexFile) => {
  const state = inspectMerge(root, branch, indexFile);
  if (state.problem !== undefined) throw new Error(state.problem);
  if (receiptPath === undefined) {
    await remoteCheck(state.head, root, state.master);
  } else {
    if (!/^target\/[A-Za-z0-9_-]+\/final-check\.json$/u.test(receiptPath))
      throw new Error("영수증 경로가 올바르지 않다");
    const receiptFile = join(root, receiptPath);
    const receipt = JSON.parse(readFileSync(receiptFile, "utf8"));
    const signers = output(root, [
      "show",
      `${state.master}:harness/allowed_signers`,
    ]);
    const principal = output(root, [
      "show",
      `${state.master}:harness/ci/signatures/owner.txt`,
    ]);
    const scratch = join(root, "target/merge-gate");
    mkdirSync(scratch, { recursive: true });
    const signed = verifyReceipt(receiptFile, signers, principal, scratch);
    if (!evidenceFor(state.head, { receipt, remote: false, signed }))
      throw new Error("검증된 서명의 SHA 일치 로컬 영수증이 아니다");
  }
  validateRemoteObjects(root, state.head);
  const check = spawnSync("just", ["check-fast-static"], {
    cwd: root,
    stdio: "inherit",
    env: environment(),
  });
  if (check.status !== 0) throw new Error("check-fast-static 실패");
  const after = inspectMerge(root, branch, indexFile);
  if (
    after.problem !== undefined ||
    after.head !== state.head ||
    after.master !== state.master ||
    after.tree !== state.tree
  ) {
    throw new Error("검사 중 병합 상태가 바뀌었다");
  }
  return state;
};

const main = async () => {
  const root = output(process.cwd(), ["rev-parse", "--show-toplevel"]);
  const marker = resolve(
    root,
    output(root, ["rev-parse", "--git-path", "stock-copilot-merge-gate.json"]),
  );
  const [branch, receipt] = process.argv.slice(2);
  if (branch === "--prepare-hook") {
    const mergeFile = output(root, ["rev-parse", "--git-path", "MERGE_HEAD"]);
    try {
      readFileSync(resolve(root, mergeFile), "utf8");
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
      rmSync(marker);
      process.stdout.write("merge-gate: 중단된 요청을 정리했다\n");
      return;
    }
    if (receipt === undefined || receipt === "")
      throw new Error("커밋 인덱스를 받지 못했다");
    const request = JSON.parse(readFileSync(marker, "utf8"));
    const index = resolve(root, receipt);
    const state = inspectMerge(root, request.branch, index);
    if (
      state.problem !== undefined ||
      state.head !== request.head ||
      state.master !== request.master ||
      state.tree !== request.tree
    )
      throw new Error("실제 커밋 인덱스가 병합 요청과 다르다");
    writeFileSync(marker, `${JSON.stringify({ ...request, index })}\n`, {
      mode: 0o600,
    });
  } else if (branch === "--hook") {
    const request = JSON.parse(readFileSync(marker, "utf8"));
    if (typeof request.index !== "string" || request.index === "")
      throw new Error("커밋 인덱스를 받지 못했다");
    const state = await validate(
      root,
      request.branch,
      request.receipt,
      request.index,
    );
    if (
      state.head !== request.head ||
      state.master !== request.master ||
      state.tree !== request.tree
    )
      throw new Error("병합 요청 이후 대상이 바뀌었다");
    rmSync(marker);
  } else {
    rmSync(marker, { force: true });
    const state = await validate(root, branch ?? "", receipt);
    writeFileSync(
      marker,
      `${JSON.stringify({ branch, receipt, ...state })}\n`,
      { flag: "wx", mode: 0o600 },
    );
  }
  process.stdout.write("merge-gate: 통과\n");
};

if (
  process.argv[1] !== undefined &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  try {
    await main();
  } catch (error) {
    process.stderr.write(
      `merge-gate: ${error instanceof Error ? error.message : "검사 실패"}\n`,
    );
    process.exitCode = 1;
  }
}
