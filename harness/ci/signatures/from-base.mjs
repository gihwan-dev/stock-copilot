import { spawnSync } from "node:child_process";
import { mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const VERIFIER = "harness/ci/signatures/verify.mjs";

export const SUMMARY = /^서명 검사: 위반 \d+건$/mu;

const OPTIONS = ["--range-start", "--base-kind", "--owner-keys"];

const REQUIRED = ["--range-start", "--base-kind"];

export const parseArguments = (args) => {
  const [repo, head, ...rest] = args;
  if (repo === undefined || head === undefined || rest.length % 2 !== 0) {
    return { problem: "인자" };
  }
  const values = {};
  for (let index = 0; index < rest.length; index += 2) {
    if (!OPTIONS.includes(rest[index]) || rest[index] in values) {
      return { problem: "인자" };
    }
    values[rest[index]] = rest[index + 1];
  }
  if (REQUIRED.some((flag) => values[flag] === undefined)) {
    return { problem: "인자" };
  }
  return {
    repo: resolve(repo),
    head,
    rangeStart: values["--range-start"],
    passed: [
      "--base-kind",
      values["--base-kind"],
      ...(values["--owner-keys"] === undefined
        ? []
        : ["--owner-keys", values["--owner-keys"]]),
    ],
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
  const trusted = based.stdout.trim();
  const scratch = realpathSync(mkdtempSync(join(tmpdir(), "signature-base-")));
  const tools = join(scratch, "base");
  try {
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
        trusted,
      ],
      { stdio: "inherit" },
    );
    if (added.status !== 0) {
      process.stderr.write(
        `검증된 커밋 ${trusted}을 대상 저장소에서 꺼내지 못했다\n`,
      );
      return 1;
    }
    const finished = spawnSync(
      "node",
      [
        join(tools, VERIFIER),
        options.repo,
        options.rangeStart,
        options.head,
        ...options.passed,
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
          "사용법: node <기준 checkout>/harness/ci/signatures/from-base.mjs <저장소> <대상 커밋> --range-start <커밋> --base-kind verified|unverified [--owner-keys <파일>]\n",
        ),
        2);
}
