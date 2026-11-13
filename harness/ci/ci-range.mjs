import { spawnSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

export const DEFAULT_BRANCH = "master";

export const ZERO = "0".repeat(40);

const COMMIT = /^[0-9a-f]{40}$/u;

const DAY_SECONDS = 24 * 60 * 60;

export const rangeFor = (event, repository) => {
  if (event.name === "pull_request") {
    return COMMIT.test(event.prBase) && COMMIT.test(event.prHead)
      ? {
          base: event.prBase,
          head: event.prHead,
          notes: [`PR: 대상 브랜치 끝 ${event.prBase} 기준`],
        }
      : { problem: "PR의 기준·대상 커밋을 알 수 없다" };
  }
  if (event.name !== "push" || !COMMIT.test(event.sha)) {
    return { problem: `기준을 정할 수 없는 이벤트: ${event.name}` };
  }
  if (event.ref === `refs/heads/${DEFAULT_BRANCH}`) {
    if (event.before === ZERO || !COMMIT.test(event.before)) {
      return {
        problem: `${DEFAULT_BRANCH}가 새로 만들어졌다(삭제 뒤 다시 만들기): 이전 끝이 없어 기준을 정할 수 없다`,
      };
    }
    if (!repository.exists(event.before)) {
      return {
        problem: `이전 ${DEFAULT_BRANCH} 끝 ${event.before}을 가져오지 못했다(강제 푸시로 사라진 커밋)`,
      };
    }
    const forced = !repository.isAncestor(event.before, event.sha);
    return {
      base: event.before,
      head: event.sha,
      notes: [
        `${DEFAULT_BRANCH} 푸시: 이전 끝 ${event.before} 기준`,
        ...(forced
          ? [`강제 푸시: ${event.before}이 ${event.sha}의 조상이 아니다`]
          : []),
      ],
    };
  }
  const tip = repository.remoteTip();
  if (tip === undefined) {
    return { problem: `origin/${DEFAULT_BRANCH}를 찾지 못했다` };
  }
  return {
    base: tip,
    head: event.sha,
    notes: [
      `${event.ref} 푸시: origin/${DEFAULT_BRANCH} 끝 ${tip} 기준${event.before === ZERO ? " (새 브랜치)" : ""}`,
    ],
  };
};

export const changedRecently = (commitSeconds, nowSeconds) =>
  nowSeconds - commitSeconds <= DAY_SECONDS ? "yes" : "no";

const git = (args) =>
  spawnSync("git", ["--no-replace-objects", ...args], { encoding: "utf8" });

const localRepository = {
  exists: (commit) =>
    git(["cat-file", "-e", `${commit}^{commit}`]).status === 0,
  isAncestor: (ancestor, descendant) =>
    git(["merge-base", "--is-ancestor", ancestor, descendant]).status === 0,
  remoteTip: () => {
    const run = git([
      "rev-parse",
      "--verify",
      `refs/remotes/origin/${DEFAULT_BRANCH}^{commit}`,
    ]);
    return run.status === 0 ? run.stdout.trim() : undefined;
  },
};

const emit = (pairs) =>
  Object.entries(pairs).forEach(([key, value]) => {
    process.stdout.write(`${key}=${value}\n`);
    if (process.env.GITHUB_OUTPUT !== undefined) {
      appendFileSync(process.env.GITHUB_OUTPUT, `${key}=${value}\n`);
    }
  });

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [command] = process.argv.slice(2);
  if (command === "base") {
    const range = rangeFor(
      {
        name: process.env.EVENT_NAME ?? "",
        ref: process.env.REF ?? "",
        before: process.env.BEFORE ?? "",
        sha: process.env.SHA ?? "",
        prBase: process.env.PR_BASE ?? "",
        prHead: process.env.PR_HEAD ?? "",
      },
      localRepository,
    );
    if (range.problem !== undefined) {
      process.stderr.write(`ci-range: ${range.problem}\n`);
      process.exitCode = 1;
    } else {
      range.notes.forEach((note) => process.stdout.write(`${note}\n`));
      emit({ base: range.base, head: range.head });
    }
  } else if (command === "changed") {
    const run = git(["log", "-1", "--format=%ct", "HEAD"]);
    emit({
      changed: changedRecently(
        Number.parseInt(run.stdout.trim(), 10),
        Math.floor(Date.now() / 1000),
      ),
    });
  } else {
    process.stderr.write(
      "사용법: node harness/ci/ci-range.mjs base | changed\n",
    );
    process.exitCode = 2;
  }
}
