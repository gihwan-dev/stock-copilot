import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { get } from "node:https";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const REPOSITORY = "gihwan-dev/stock-copilot";
export const COMMIT = /^[0-9a-f]{40}$/u;
export const WORKFLOWS = [
  { file: "ci.yml", jobs: ["linux"] },
  { file: "signatures.yml", jobs: ["signatures"] },
];
const REQUIRED_STEPS = {
  linux: [
    "container visual regression",
    "linux check",
    "linux tests without network",
  ],
  macos: ["just check", "diff mutants"],
  "macos-branch": ["just check", "diff mutants"],
  signatures: ["signatures from the base checkout"],
};

export const checkRun = (sha, run, jobs, workflow = WORKFLOWS[0]) => {
  if (
    !COMMIT.test(sha) ||
    run?.head_sha !== sha ||
    run.event !== "push" ||
    run.path !== `.github/workflows/${workflow.file}` ||
    run.repository?.full_name !== REPOSITORY ||
    run.head_repository?.full_name !== REPOSITORY ||
    typeof run.head_branch !== "string" ||
    run.head_branch === "" ||
    run.status !== "completed" ||
    run.conclusion !== "success" ||
    !Number.isSafeInteger(run.id) ||
    !Number.isSafeInteger(run.run_attempt) ||
    !Array.isArray(jobs)
  ) {
    return false;
  }
  const required =
    workflow.file === "ci.yml"
      ? [
          ...workflow.jobs,
          run.head_branch === "master" ? "macos" : "macos-branch",
        ]
      : workflow.jobs;
  return required.every((name) => {
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
  });
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

export const remoteCheckFrom = async (sha, read, report = () => {}) => {
  if (!COMMIT.test(sha)) throw new Error("40자리 소문자 커밋 SHA가 필요하다");
  for (const workflow of WORKFLOWS) {
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
    const candidates = data.workflow_runs.filter(
      (run) => run.head_sha === sha && run.event === "push",
    );
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
    if (
      !Array.isArray(result.jobs) ||
      result.total_count !== result.jobs.length ||
      !checkRun(sha, run, result.jobs, workflow)
    ) {
      throw new Error(
        `${workflow.file}: 해당 SHA의 필수 작업이 모두 성공하지 않았다`,
      );
    }
    report(`${workflow.file}: 성공, 실행 ${run.id}, 회차 ${run.run_attempt}\n`);
  }
  return true;
};

export const remoteCheck = (sha) =>
  remoteCheckFrom(sha, request, (line) => process.stdout.write(line));

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
    await remoteCheck(process.argv[2] ?? "");
    process.stdout.write("remote-check: 통과\n");
  } catch (error) {
    process.stderr.write(
      `remote-check: ${error instanceof Error ? error.message : "조회 실패"}\n`,
    );
    process.exitCode = 1;
  }
}
