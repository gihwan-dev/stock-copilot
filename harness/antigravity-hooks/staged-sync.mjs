import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { gitEnvironment } from "../../.claude/hooks/lib/project.mjs";

export const MASTER_REF = "origin/master";
export const BLOCKED_FILE = "stock-copilot-blocked.json";
export const PATCH_FILE = "stock-copilot-staged.patch";
const BLOCKED_PREFIX = "막힘:";

export const runGit = (root, args, input) => {
  const result = spawnSync("git", ["-C", root, ...args], {
    env: gitEnvironment(),
    input,
    maxBuffer: 256 * 1024 * 1024,
  });
  return {
    status: result.status ?? 1,
    stdout: result.stdout ?? Buffer.alloc(0),
    stderr: String(result.stderr ?? ""),
  };
};

const text = (root, args) => {
  const result = runGit(root, args);
  return result.status === 0 ? String(result.stdout).trim() : undefined;
};

export const gitDirectory = (root) => {
  const found = text(root, ["rev-parse", "--git-dir"]);
  if (found === undefined || found === "") {
    return undefined;
  }
  return isAbsolute(found) ? found : resolve(root, found);
};

export const stateDigest = (root) => {
  const hash = createHash("sha256");
  const parts = [
    ["rev-parse", "HEAD"],
    ["status", "--porcelain=v1", "-uall", "-z"],
    ["diff", "--binary", "HEAD"],
    ["diff", "--binary", "--cached"],
  ];
  for (const args of parts) {
    const result = runGit(root, args);
    if (result.status !== 0) {
      return undefined;
    }
    hash.update(result.stdout);
    hash.update("\0");
  }
  const untracked = runGit(root, [
    "ls-files",
    "--others",
    "--exclude-standard",
    "-z",
  ]);
  if (untracked.status !== 0) {
    return undefined;
  }
  for (const name of String(untracked.stdout).split("\0").filter(Boolean)) {
    hash.update(name);
    hash.update("\0");
    try {
      hash.update(readFileSync(join(root, name)));
    } catch {
      return undefined;
    }
    hash.update("\0");
  }
  return hash.digest("hex");
};

export const recordBlocked = (root, reason) => {
  const line = String(reason ?? "").trim();
  if (line === "" || line.includes("\n")) {
    return { ok: false, message: "막힘 이유는 비지 않은 한 줄이어야 한다" };
  }
  const directory = gitDirectory(root);
  const head = text(root, ["rev-parse", "HEAD"]);
  const digest = stateDigest(root);
  if (directory === undefined || head === undefined || digest === undefined) {
    return { ok: false, message: "git 상태를 읽지 못했다 (닫힌 실패)" };
  }
  writeFileSync(
    join(directory, BLOCKED_FILE),
    `${JSON.stringify({ report: `${BLOCKED_PREFIX} ${line}`, head, digest })}\n`,
  );
  return { ok: true, message: `${BLOCKED_PREFIX} ${line}` };
};

export const blockedExit = (root) => {
  const directory = gitDirectory(root);
  if (directory === undefined) {
    return false;
  }
  let record;
  try {
    record = JSON.parse(readFileSync(join(directory, BLOCKED_FILE), "utf8"));
  } catch {
    return false;
  }
  const head = text(root, ["rev-parse", "HEAD"]);
  return (
    typeof record?.report === "string" &&
    record.report.startsWith(`${BLOCKED_PREFIX} `) &&
    record.report.trim().length > BLOCKED_PREFIX.length &&
    head !== undefined &&
    record.head === head &&
    record.digest === stateDigest(root)
  );
};

export const clearBlocked = (root) => {
  const directory = gitDirectory(root);
  if (directory !== undefined) {
    rmSync(join(directory, BLOCKED_FILE), { force: true });
  }
};

const failure = (message) => ({ ok: false, message });

const preconditions = (root) => {
  const directory = gitDirectory(root);
  if (directory === undefined) {
    return failure("git 저장소가 아니다");
  }
  const branch = text(root, ["symbolic-ref", "--quiet", "--short", "HEAD"]);
  if (branch === undefined || branch === "" || branch === "master") {
    return failure("이름 있는 작업 브랜치에서만 쓴다");
  }
  for (const marker of [
    "MERGE_HEAD",
    "CHERRY_PICK_HEAD",
    "REVERT_HEAD",
    "rebase-merge",
    "rebase-apply",
  ]) {
    if (existsSync(join(directory, marker))) {
      return failure(`진행 중인 git 작업이 있다: ${marker}`);
    }
  }
  if (runGit(root, ["diff", "--quiet"]).status !== 0) {
    return failure(
      "스테이징하지 않은 추적 파일 변경이 있다. 스테이징하거나 되돌린 뒤 다시 실행하라",
    );
  }
  const untracked = text(root, ["ls-files", "--others", "--exclude-standard"]);
  if (untracked === undefined || untracked !== "") {
    return failure(
      "추적하지 않은 파일이 있다. 스테이징하거나 지운 뒤 다시 실행하라",
    );
  }
  if (
    text(root, [
      "rev-parse",
      "--verify",
      "--quiet",
      `${MASTER_REF}^{commit}`,
    ]) === undefined
  ) {
    return failure(`${MASTER_REF}가 없다`);
  }
  return { ok: true, directory };
};

const restore = (root, head, patch) => {
  runGit(root, ["reset", "-q", "--hard", head]);
  if (patch.length === 0) {
    return true;
  }
  return (
    runGit(root, ["apply", "--index", "--binary", "-"], patch).status === 0
  );
};

export const syncMaster = (root) => {
  const ready = preconditions(root);
  if (!ready.ok) {
    return ready;
  }
  const head = text(root, ["rev-parse", "HEAD"]);
  const staged = runGit(root, ["diff", "--cached", "--binary"]);
  if (head === undefined || staged.status !== 0) {
    return failure("HEAD나 스테이징 변경을 읽지 못했다");
  }
  const patch = staged.stdout;
  const saved = join(ready.directory, PATCH_FILE);
  writeFileSync(saved, patch);
  if (runGit(root, ["reset", "-q", "--hard", head]).status !== 0) {
    restore(root, head, patch);
    return failure(
      `작업 사본을 HEAD로 맞추지 못했다. 스테이징 변경은 ${saved}에 있다`,
    );
  }
  const merged = runGit(root, ["merge", "--no-ff", "--no-edit", MASTER_REF]);
  if (merged.status !== 0) {
    runGit(root, ["merge", "--abort"]);
    const restored = restore(root, head, patch);
    return failure(
      restored
        ? `${MASTER_REF} 병합이 충돌해 되돌렸다. 스테이징 변경을 복원했다`
        : `${MASTER_REF} 병합이 충돌했고 스테이징 복원도 실패했다. 변경은 ${saved}에 있다`,
    );
  }
  if (patch.length > 0) {
    const applied = runGit(
      root,
      ["apply", "--index", "--binary", "--3way", "-"],
      patch,
    );
    if (applied.status !== 0) {
      const restored = restore(root, head, patch);
      return failure(
        restored
          ? `병합 뒤 스테이징 변경이 겹쳐 병합을 되돌렸다. 스테이징 변경을 복원했다`
          : `스테이징 복원에 실패했다. 변경은 ${saved}에 있다`,
      );
    }
  }
  rmSync(saved, { force: true });
  return {
    ok: true,
    message: `${MASTER_REF}를 병합하고 스테이징 변경을 복원했다`,
  };
};

const main = (argv) => {
  const [command, ...rest] = argv;
  const root = process.cwd();
  const outcome =
    command === "sync"
      ? syncMaster(root)
      : command === "blocked"
        ? recordBlocked(root, rest.join(" "))
        : failure("사용법: staged-sync.mjs sync | blocked <한 줄 이유>");
  process.stdout.write(`${outcome.message}\n`);
  return outcome.ok ? 0 : 1;
};

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  process.exitCode = main(process.argv.slice(2));
}
