import { spawnSync } from "node:child_process";
import { mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const VERIFIER = "harness/ci/signatures/verify.mjs";

export const SUMMARY = /^서명 검사: 위반 \d+건$/mu;

export const OWNER_KEYS_URL = (owner) =>
  `https://api.github.com/users/${encodeURIComponent(owner)}/ssh_signing_keys`;

export const ownerKeysFromResponse = (text) => {
  try {
    const parsed = JSON.parse(text);
    return Array.isArray(parsed)
      ? {
          keys: parsed
            .map((entry) =>
              typeof entry?.key === "string"
                ? entry.key.trim().split(/\s+/u).slice(0, 2).join(" ")
                : "",
            )
            .filter((key) => key !== ""),
        }
      : { problem: "소유자 키 응답이 배열이 아니다" };
  } catch {
    return { problem: "소유자 키 응답을 읽지 못했다" };
  }
};

const fetchOwnerKeys = (owner) => {
  const finished = spawnSync(
    "curl",
    [
      "-sSfL",
      "--max-time",
      "30",
      "-H",
      "Accept: application/vnd.github+json",
      OWNER_KEYS_URL(owner),
    ],
    { encoding: "utf8" },
  );
  return finished.status === 0
    ? ownerKeysFromResponse(finished.stdout)
    : { problem: `소유자 키를 가져오지 못했다: ${finished.stderr.trim()}` };
};

export const parseArguments = (args) => {
  const [repo, head, flag, value, ...rest] = args;
  if (
    repo === undefined ||
    head === undefined ||
    value === undefined ||
    rest.length > 0 ||
    !["--owner", "--owner-keys"].includes(flag)
  ) {
    return { problem: "인자" };
  }
  return {
    repo: resolve(repo),
    head,
    [flag === "--owner" ? "owner" : "keysFile"]: value,
  };
};

const toolsRoot = resolve(
  fileURLToPath(import.meta.url),
  "..",
  "..",
  "..",
  "..",
);

export const runFromBase = (options) => {
  const based = spawnSync("git", ["-C", toolsRoot, "rev-parse", "HEAD"], {
    encoding: "utf8",
  });
  if (based.status !== 0) {
    process.stderr.write("기준 checkout의 커밋을 읽지 못했다\n");
    return 1;
  }
  const base = based.stdout.trim();
  const scratch = realpathSync(mkdtempSync(join(tmpdir(), "signature-base-")));
  const tools = join(scratch, "base");
  try {
    const keys =
      options.keysFile === undefined
        ? fetchOwnerKeys(options.owner)
        : { file: options.keysFile };
    if (keys.problem !== undefined) {
      process.stderr.write(`${keys.problem}\n`);
      return 1;
    }
    const keysFile = keys.file ?? join(scratch, "owner-keys.txt");
    if (keys.file === undefined) {
      writeFileSync(keysFile, keys.keys.map((key) => `${key}\n`).join(""));
    }
    const added = spawnSync(
      "git",
      [
        "-C",
        options.repo,
        "worktree",
        "add",
        "--quiet",
        "--detach",
        tools,
        base,
      ],
      { stdio: "inherit" },
    );
    if (added.status !== 0) {
      process.stderr.write(
        `기준 커밋 ${base}을 대상 저장소에서 꺼내지 못했다\n`,
      );
      return 1;
    }
    const finished = spawnSync(
      "node",
      [
        join(tools, VERIFIER),
        options.repo,
        base,
        options.head,
        "--owner-keys",
        keysFile,
      ],
      { cwd: tools, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 },
    );
    process.stdout.write(finished.stdout);
    process.stderr.write(finished.stderr);
    if (!SUMMARY.test(finished.stdout)) {
      process.stderr.write("서명 검사가 요약 줄 없이 끝났다\n");
      return 1;
    }
    return finished.status ?? 1;
  } finally {
    spawnSync(
      "git",
      ["-C", options.repo, "worktree", "remove", "--force", tools],
      { stdio: "ignore" },
    );
    rmSync(scratch, { recursive: true, force: true });
  }
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const options = parseArguments(process.argv.slice(2));
  process.exitCode =
    options.problem === undefined
      ? runFromBase(options)
      : (process.stderr.write(
          "사용법: node <기준 checkout>/harness/ci/signatures/from-base.mjs <저장소> <대상 커밋> (--owner <소유자> | --owner-keys <파일>)\n",
        ),
        2);
}
