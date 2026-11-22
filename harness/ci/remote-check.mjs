import { spawnSync } from "node:child_process";
import { readFileSync, rmSync, writeFileSync } from "node:fs";
import { mkdtempSync } from "./managed-temp.mjs";
import { get } from "node:https";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  isWritable,
  parseAllowlist,
} from "../../.claude/hooks/lib/allowlist.mjs";

export const REPOSITORY = "gihwan-dev/stock-copilot";
export const COMMIT = /^[0-9a-f]{40}$/u;
export const WORKFLOWS = [
  { file: "ci.yml", jobs: ["linux"] },
  { file: "signatures.yml", jobs: ["signatures"] },
];

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

const trustedGit = (root, args) =>
  spawnSync(
    "git",
    ["--no-replace-objects", "-C", root, "-c", "core.fsmonitor=false", ...args],
    {
      encoding: "utf8",
      env: {
        ...Object.fromEntries(
          Object.entries(process.env).filter(([key]) => !REDIRECTING.has(key)),
        ),
        GIT_CONFIG_GLOBAL: "/dev/null",
        GIT_CONFIG_NOSYSTEM: "1",
        GIT_GRAFT_FILE: "/dev/null",
      },
      maxBuffer: 16 * 1024 * 1024,
    },
  );

const definitionFile = (path) =>
  /(?:^|\/)(?:Cargo\.toml|Cargo\.lock|build\.rs|package\.json|pnpm-lock\.yaml|pnpm-workspace\.yaml|justfile|lefthook\.ya?ml|\.node-version|rust-toolchain(?:\.toml)?)$/u.test(
    path,
  );

export const assertTrustedExecution = (root, base, head) => {
  if (!COMMIT.test(base) || !COMMIT.test(head))
    throw new Error("검사 정의 기준과 대상은 40자리 SHA여야 한다");
  const baseline = trustedGit(root, [
    "show",
    `${base}:harness/agent-writable.txt`,
  ]);
  const allowlist =
    baseline.status === 0 ? parseAllowlist(baseline.stdout) : {};
  const changes = trustedGit(root, [
    "diff",
    "--name-only",
    "-z",
    "--no-renames",
    "--no-ext-diff",
    "--no-textconv",
    "--ignore-submodules=none",
    base,
    head,
    "--",
  ]);
  if (
    allowlist.problem !== undefined ||
    !Array.isArray(allowlist.includes) ||
    changes.status !== 0
  )
    throw new Error("검사 정의의 기준 또는 대상 트리를 확인하지 못했다");
  const untrusted = changes.stdout
    .split("\0")
    .filter(
      (path) =>
        path !== "" && (!isWritable(allowlist, path) || definitionFile(path)),
    );
  if (untrusted.length !== 0)
    throw new Error(
      `검사 정의 또는 보호 경로가 기준 master와 다르다: ${untrusted.join(", ")}`,
    );
};

const masterAt = (root) => {
  const result = trustedGit(root, [
    "rev-parse",
    "--verify",
    "refs/heads/master^{commit}",
  ]);
  const master = result.stdout.trim();
  if (result.status !== 0 || !COMMIT.test(master))
    throw new Error("검사 정의의 master 기준을 확인하지 못했다");
  return master;
};
const REQUIRED_STEPS = {
  linux: [
    "container visual regression",
    "linux check",
    "linux tests without network",
  ],
  macos: ["just check", "diff mutants", "diff mutants verdict"],
  "macos-branch": [
    "branch checkout",
    "just check",
    "diff mutants",
    "diff mutants verdict",
  ],
  signatures: ["signatures from the base checkout"],
};

const invalidRun = (sha, run, workflow) =>
  !COMMIT.test(sha) ||
  run?.head_sha !== sha ||
  run.event !== "push" ||
  run.path !== `.github/workflows/${workflow.file}` ||
  run.repository?.full_name !== REPOSITORY ||
  run.head_repository?.full_name !== REPOSITORY ||
  typeof run.head_branch !== "string" ||
  run.head_branch === "" ||
  !Number.isSafeInteger(run.id) ||
  !Number.isSafeInteger(run.run_attempt);

const requiredJobs = (run, workflow) =>
  workflow.file === "ci.yml"
    ? [
        ...workflow.jobs,
        run.head_branch === "master" ? "macos" : "macos-branch",
      ]
    : workflow.jobs;

const completeJob = (sha, run, jobs, name) => {
  const matches = jobs.filter((job) => job.name === name);
  return (
    matches.length === 1 &&
    matches[0].status === "completed" &&
    matches[0].conclusion === "success" &&
    matches[0].head_sha === sha &&
    matches[0].run_attempt === run.run_attempt &&
    Array.isArray(matches[0].steps) &&
    REQUIRED_STEPS[name].every((step) => {
      const found = matches[0].steps.filter((entry) => entry.name === step);
      return (
        found.length === 1 &&
        found[0].status === "completed" &&
        found[0].conclusion === "success"
      );
    })
  );
};

const missingRequiredSteps = (sha, run, jobs, workflow) => {
  if (
    invalidRun(sha, run, workflow) ||
    run.status !== "completed" ||
    run.conclusion !== "success" ||
    !Array.isArray(jobs)
  )
    return [];
  const entries = requiredJobs(run, workflow).map((name) => ({
    name,
    jobs: jobs.filter((job) => job.name === name),
  }));
  if (
    entries.some(
      ({ jobs: matches }) =>
        matches.length !== 1 ||
        matches[0].status !== "completed" ||
        matches[0].conclusion !== "success" ||
        matches[0].head_sha !== sha ||
        matches[0].run_attempt !== run.run_attempt ||
        !Array.isArray(matches[0].steps),
    )
  )
    return [];
  return entries.flatMap(({ name, jobs: [entry] }) =>
    REQUIRED_STEPS[name]
      .filter((step) => !entry.steps.some((item) => item.name === step))
      .map((step) => `${name}: ${step}`),
  );
};

export const checkRun = (sha, run, jobs, workflow = WORKFLOWS[0]) => {
  if (
    invalidRun(sha, run, workflow) ||
    run.status !== "completed" ||
    run.conclusion !== "success" ||
    !Array.isArray(jobs)
  )
    return false;
  return requiredJobs(run, workflow).every((name) =>
    completeJob(sha, run, jobs, name),
  );
};

export const checkLinuxRun = (sha, run, jobs) => {
  if (
    invalidRun(sha, run, WORKFLOWS[0]) ||
    ![
      "queued",
      "requested",
      "waiting",
      "pending",
      "in_progress",
      "completed",
    ].includes(run.status) ||
    !Array.isArray(jobs)
  )
    return false;
  return completeJob(sha, run, jobs, "linux");
};

export const failureCode = (sha, run, jobs, workflow = WORKFLOWS[0]) => {
  if (invalidRun(sha, run, workflow) || !Array.isArray(jobs))
    return "INVALID_EVIDENCE";
  if (
    ["queued", "requested", "waiting", "pending", "in_progress"].includes(
      run.status,
    )
  )
    return "PENDING";
  if (run.status !== "completed") return "INVALID_EVIDENCE";
  const required = requiredJobs(run, workflow);
  const entries = required.map((name) =>
    jobs.filter((job) => job.name === name),
  );
  if (
    entries.some(
      (items) =>
        items.length !== 1 ||
        items[0].head_sha !== sha ||
        items[0].run_attempt !== run.run_attempt,
    )
  ) {
    return run.conclusion === "cancelled"
      ? "RETRY_CANCELLED"
      : "INVALID_EVIDENCE";
  }
  const failed = entries
    .map((items) => items[0])
    .filter((job) => job.conclusion !== "success");
  if (failed.some((job) => job.status !== "completed"))
    return "INVALID_EVIDENCE";
  const unassigned = (job) =>
    (job.runner_id === 0 || job.runner_id === null) &&
    (job.runner_name === "" || job.runner_name === null) &&
    Array.isArray(job.steps) &&
    job.steps.length === 0;
  const uncertainAssignment = (job) =>
    (job.runner_id === 0 || job.runner_id === null) &&
    (job.runner_name === "" || job.runner_name === null) &&
    job.steps === null;
  const timedOutStep = (job) =>
    Array.isArray(job.steps) &&
    job.steps.some((step) => step.conclusion === "timed_out");
  const failedStep = (job) =>
    Array.isArray(job.steps) &&
    job.steps.some((step) => step.conclusion === "failure");
  const mutationLimit = (job, stepName) => {
    if (
      !["macos", "macos-branch"].includes(job.name) ||
      job.conclusion !== "failure" ||
      !Array.isArray(job.steps)
    )
      return false;
    const failures = job.steps.filter((step) => step.conclusion === "failure");
    const passed = (name) =>
      job.steps.filter(
        (step) =>
          step.name === name &&
          step.status === "completed" &&
          step.conclusion === "success",
      ).length === 1;
    return (
      failures.length === 1 &&
      failures[0].name === stepName &&
      failures[0].status === "completed" &&
      passed("just check") &&
      passed("diff mutants") &&
      (job.name !== "macos-branch" || passed("branch checkout"))
    );
  };
  if (
    failed.length > 0 &&
    failed.every((job) => mutationLimit(job, "diff mutants timeout"))
  )
    return "RETRY_MUTATION_TIMED_OUT";
  if (
    failed.length > 0 &&
    failed.every((job) => mutationLimit(job, "diff mutants budget"))
  )
    return "RETRY_MUTATION_BUDGET";
  if (
    failed.some(
      (job) =>
        job.conclusion === "failure" &&
        !unassigned(job) &&
        !uncertainAssignment(job) &&
        (!timedOutStep(job) || failedStep(job)),
    )
  )
    return "CHECK_FAILED";
  if (
    failed.some(
      (job) => job.conclusion === "failure" && uncertainAssignment(job),
    )
  )
    return "INVALID_EVIDENCE";
  if (
    failed.some(
      (job) =>
        unassigned(job) &&
        ["failure", "cancelled", "timed_out"].includes(job.conclusion),
    )
  )
    return "RETRY_RUNNER_UNASSIGNED";
  if (
    failed.some((job) => job.conclusion === "timed_out" || timedOutStep(job)) ||
    run.conclusion === "timed_out"
  )
    return "RETRY_TIMED_OUT";
  if (
    failed.some((job) => job.conclusion === "cancelled") ||
    run.conclusion === "cancelled"
  )
    return "RETRY_CANCELLED";
  if (run.conclusion === "failure") return "CHECK_FAILED";
  if (missingRequiredSteps(sha, run, jobs, workflow).length !== 0)
    return "MISSING_REQUIRED_STEPS";
  return "INVALID_EVIDENCE";
};

export const evidenceFor = (sha, { receipt, remote, signed }) => {
  if (!COMMIT.test(sha)) return false;
  if (
    receipt !== undefined &&
    (receipt === null ||
      receipt.head !== sha ||
      receipt.recipe !== "check" ||
      receipt.exit !== 0)
  )
    return false;
  return remote === true || (receipt !== undefined && signed === true);
};

const request = (path) => {
  const token = process.env.GH_TOKEN ?? process.env.GITHUB_TOKEN;
  return new Promise((resolveRequest, reject) => {
    const call = get(
      `https://api.github.com/repos/${REPOSITORY}/${path}`,
      {
        headers: {
          Accept: "application/vnd.github+json",
          "User-Agent": "stock-copilot-remote-check",
          ...(token === undefined || token === ""
            ? {}
            : { Authorization: `Bearer ${token}` }),
        },
      },
      (response) => {
        if (response.statusCode !== 200) {
          response.resume();
          reject(new Error(`GitHub API HTTP ${response.statusCode}`));
          return;
        }
        let body = "";
        response.setEncoding("utf8");
        response.on("data", (chunk) => {
          body += chunk;
          if (body.length > 10 * 1024 * 1024)
            call.destroy(new Error("GitHub API 응답이 너무 크다"));
        });
        response.on("error", reject);
        response.on("end", () => {
          try {
            resolveRequest(JSON.parse(body));
          } catch {
            reject(new Error("GitHub API 응답을 읽지 못했다"));
          }
        });
      },
    );
    call.setTimeout(30000, () =>
      call.destroy(new Error("GitHub API 시간 초과")),
    );
    call.on("error", reject);
  });
};

const workflowResult = async (sha, read, workflow) => {
  const data = await read(
    `actions/workflows/${workflow.file}/runs?head_sha=${sha}&event=push&per_page=100`,
  );
  if (
    !Array.isArray(data.workflow_runs) ||
    data.total_count !== data.workflow_runs.length ||
    data.workflow_runs.length === 0
  ) {
    throw new Error(
      `${workflow.file}: 해당 SHA의 실행이 없거나 응답이 불완전하다`,
    );
  }
  const candidates = data.workflow_runs;
  if (
    candidates.some(
      (run) =>
        run?.head_sha !== sha ||
        run.event !== "push" ||
        !Number.isSafeInteger(run.id) ||
        run.id <= 0 ||
        !Number.isSafeInteger(run.run_attempt) ||
        run.run_attempt <= 0,
    )
  )
    throw new Error(`${workflow.file}: 실행 ID 또는 회차가 없다`);
  candidates.sort((a, b) => b.id - a.id);
  const run = candidates[0];
  if (
    run === undefined ||
    !Number.isSafeInteger(run.id) ||
    !Number.isSafeInteger(run.run_attempt)
  ) {
    throw new Error(`${workflow.file}: 실행 ID 또는 회차가 없다`);
  }
  const result = await read(
    `actions/runs/${run.id}/attempts/${run.run_attempt}/jobs?per_page=100`,
  );
  if (!Array.isArray(result.jobs) || result.total_count !== result.jobs.length)
    throw new Error(`${workflow.file}: 작업 응답이 불완전하다`);
  return { run, jobs: result.jobs };
};

const FAILURE_MESSAGES = {
  RETRY_RUNNER_UNASSIGNED: "러너 미배정 상태로 종료되어 재시도 대상이다",
  RETRY_CANCELLED: "작업이 취소되어 재시도 대상이다",
  RETRY_TIMED_OUT:
    "작업이 시간 초과되어 재시도 대상이다; 원인은 확인되지 않았다",
  RETRY_MUTATION_TIMED_OUT:
    "변경분 변이 조각이 20분 제한에 도달해 재시도 또는 추가 분할 대상이다",
  RETRY_MUTATION_BUDGET:
    "변경분 변이의 다음 조각을 실행하기에 작업 시간이 부족해 재시도 또는 변경 분할 대상이다",
  PENDING: "작업이 아직 진행 중이다",
  CHECK_FAILED: "검사 또는 준비 단계가 실패했다; 실행 로그를 확인한다",
  MISSING_REQUIRED_STEPS: "성공 실행에 현재 필수 단계가 없다",
  INVALID_EVIDENCE: "실행 출처나 필수 작업 증거가 올바르지 않다",
};

const rejectRun = (sha, run, jobs, workflow) => {
  const code = failureCode(sha, run, jobs, workflow);
  const missing = missingRequiredSteps(sha, run, jobs, workflow);
  const error = new Error(
    `${workflow.file}: ${code}: ${FAILURE_MESSAGES[code]}${missing.length === 0 ? "" : ` (${missing.join(", ")})`}`,
  );
  error.code = code;
  throw error;
};

export const remoteCheckFrom = async (sha, read, report = () => {}) => {
  if (!COMMIT.test(sha)) throw new Error("40자리 소문자 커밋 SHA가 필요하다");
  for (const workflow of WORKFLOWS) {
    const { run, jobs } = await workflowResult(sha, read, workflow);
    if (!checkRun(sha, run, jobs, workflow))
      rejectRun(sha, run, jobs, workflow);
    report(`${workflow.file}: 성공, 실행 ${run.id}, 회차 ${run.run_attempt}\n`);
  }
  return true;
};

export const remoteLinuxCheckFrom = async (sha, read, report = () => {}) => {
  if (!COMMIT.test(sha)) throw new Error("40자리 소문자 커밋 SHA가 필요하다");
  const workflow = WORKFLOWS[0];
  const { run, jobs } = await workflowResult(sha, read, workflow);
  if (!checkLinuxRun(sha, run, jobs)) rejectRun(sha, run, jobs, workflow);
  report(
    `ci.yml: Linux 범위 성공, 실행 ${run.id}, 회차 ${run.run_attempt} (전체 검사가 아니다)\n`,
  );
  return { scope: "linux", head: sha, runId: run.id, attempt: run.run_attempt };
};

export const remoteCheck = (
  sha,
  root = process.cwd(),
  base = masterAt(root),
) => {
  assertTrustedExecution(root, base, sha);
  return remoteCheckFrom(sha, request, (line) => process.stdout.write(line));
};

export const remoteLinuxCheck = (
  sha,
  root = process.cwd(),
  base = masterAt(root),
) => {
  assertTrustedExecution(root, base, sha);
  return remoteLinuxCheckFrom(sha, request, (line) =>
    process.stdout.write(line),
  );
};

export const verifyReceipt = (receiptPath, signers, principal, scratch) => {
  if (signers.trim() === "" || !/^[A-Za-z0-9-]+$/u.test(principal))
    return false;
  const directory = mkdtempSync(join(scratch, "receipt-"));
  try {
    const signersPath = join(directory, "allowed_signers");
    writeFileSync(signersPath, signers, { mode: 0o600 });
    const input = readFileSync(receiptPath);
    const result = spawnSync(
      "ssh-keygen",
      [
        "-Y",
        "verify",
        "-f",
        signersPath,
        "-I",
        principal,
        "-n",
        "stock-copilot-check",
        "-s",
        `${receiptPath}.sig`,
      ],
      {
        input,
        stdio: ["pipe", "ignore", "ignore"],
      },
    );
    return result.status === 0;
  } catch {
    return false;
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
};

if (
  process.argv[1] !== undefined &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  try {
    const mode = process.argv[3];
    if (process.argv.length > 4 || (mode !== undefined && mode !== "--linux"))
      throw new Error("사용법: remote-check <sha> [--linux]");
    if (mode === "--linux") {
      await remoteLinuxCheck(process.argv[2] ?? "");
      process.stdout.write("remote-check: Linux 범위 통과 (병합 판정 아님)\n");
    } else {
      await remoteCheck(process.argv[2] ?? "");
      process.stdout.write("remote-check: 통과\n");
    }
  } catch (error) {
    process.stderr.write(
      `remote-check: ${error instanceof Error ? error.message : "조회 실패"}\n`,
    );
    process.exitCode = 1;
  }
}
