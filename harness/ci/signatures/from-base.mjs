import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const VERIFIER = "harness/ci/signatures/verify.mjs";

export const SUMMARY = /^서명 검사: 위반 \d+건$/mu;

export const verifierCommand = (tools, repo, base, head) => [
  "node",
  [join(tools, VERIFIER), repo, base, head],
];

export const runFromBase = (repo, base, head) => {
  const scratch = realpathSync(mkdtempSync(join(tmpdir(), "signature-base-")));
  const tools = join(scratch, "base");
  try {
    const added = spawnSync(
      "git",
      ["-C", repo, "worktree", "add", "--quiet", "--detach", tools, base],
      { stdio: "inherit" },
    );
    if (added.status !== 0) {
      process.stderr.write(`기준 커밋 ${base}을 꺼내지 못했다\n`);
      return 1;
    }
    if (!existsSync(join(tools, VERIFIER))) {
      process.stdout.write(
        `서명 검사 비활성: 기준 커밋 ${base}에 ${VERIFIER}가 없다\n`,
      );
      return 0;
    }
    const [program, args] = verifierCommand(tools, repo, base, head);
    const finished = spawnSync(program, args, {
      cwd: tools,
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
    });
    process.stdout.write(finished.stdout);
    process.stderr.write(finished.stderr);
    if (!SUMMARY.test(finished.stdout)) {
      process.stderr.write("서명 검사가 요약 줄 없이 끝났다\n");
      return 1;
    }
    return finished.status ?? 1;
  } finally {
    spawnSync("git", ["-C", repo, "worktree", "remove", "--force", tools], {
      stdio: "ignore",
    });
    rmSync(scratch, { recursive: true, force: true });
  }
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [repo, base, head] = process.argv.slice(2);
  process.exitCode =
    repo === undefined || base === undefined || head === undefined
      ? (process.stderr.write(
          "사용법: node harness/ci/signatures/from-base.mjs <저장소> <기준 커밋> <대상 커밋>\n",
        ),
        2)
      : runFromBase(resolve(repo), base, head);
}
