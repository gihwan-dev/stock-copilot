import { isWritable } from "./allowlist.mjs";
import { LOCK } from "./project.mjs";
import { STATUS } from "./state.mjs";

export const REPORT = Object.freeze({
  done: "done",
  blocked: "blocked",
  invalid: "invalid",
});

export const reportKind = (message, rules) => {
  const first = (typeof message === "string" ? message : "")
    .split("\n")[0]
    .trim();
  if (first === rules.reportFirstLines.done) {
    return REPORT.done;
  }
  return first.startsWith(rules.reportFirstLines.blocked) &&
    first.length > rules.reportFirstLines.blocked.length
    ? REPORT.blocked
    : REPORT.invalid;
};

export const reportLinesOf = (message, prompt, rules) =>
  [
    ...new Set(
      (typeof message === "string" ? message : "")
        .split("\n")
        .map((line) => line.trim())
        .filter(
          (line) =>
            line.length >= rules.reportLineMinimum &&
            !(typeof prompt === "string" ? prompt : "").includes(line),
        ),
    ),
  ].slice(0, 200);

export const protectedChanges = (changed, allowlist, existingAdrs) =>
  changed
    .filter(
      (path) =>
        allowlist.problem !== undefined ||
        !isWritable(allowlist, path) ||
        existingAdrs.includes(path),
    )
    .sort();

const FIRST_LINE_RULE =
  '보고 첫 줄은 정확히 "완료" 또는 "막힘: <한 줄 이유>"다 (docs/workflow.md 2.2).';

const tail = (text, limit) => (text.length > limit ? text.slice(-limit) : text);

export const inspectWorktree = (record, facts) => {
  const worktree = facts.locateWorktree(record.branch);
  if (worktree === undefined) {
    return {
      problems: [
        `브리프의 브랜치 ${record.branch ?? "(없음)"}가 체크아웃된 작업 사본을 찾지 못했다`,
      ],
    };
  }
  const head = facts.headOf(worktree);
  if (head === undefined) {
    return { worktree, problems: [`${worktree}의 HEAD를 읽지 못했다`] };
  }
  const problems = facts.isAncestor(worktree, record.base, head)
    ? []
    : [`기준 커밋 ${record.base}이 HEAD ${head}의 조상이 아니다 (ADR 0013)`];
  const changes = protectedChanges(
    facts.changedSince(worktree, record.base),
    facts.allowlist,
    facts.existingAdrs(worktree, record.base),
  );
  return { worktree, head, problems, protectedChanges: changes };
};

export const decideImplementerStop = (input, record, facts, rules) => {
  if (record === undefined || record.base === undefined) {
    const reason =
      record?.reason ?? "시작 기록(기준 커밋)이 없어 검사하지 않았다";
    return {
      record: {
        kind: "implementer",
        agentId: input.agent_id,
        sessionId: input.session_id,
        ...record,
        status: STATUS.unchecked,
        reason,
        updatedAt: facts.now(),
      },
      output: {
        systemMessage: `implementer ${input.agent_id}: just check를 돌리지 않았다 (unchecked): ${reason}`,
      },
    };
  }
  const report = reportKind(input.last_assistant_message, rules);
  const inspected = inspectWorktree(record, facts);
  const transcribed = facts.briefDigest(input);
  const briefProblems =
    transcribed === undefined || transcribed === record.promptDigest
      ? []
      : ["서브에이전트가 받은 브리프가 시작 때 기록한 브리프와 다르다"];
  const lockedChanges =
    facts.lock === LOCK.off ? [] : (inspected.protectedChanges ?? []);
  const problems = [
    ...briefProblems,
    ...inspected.problems,
    ...lockedChanges.map((path) => `보호 경로 변경: ${path}`),
  ];
  const ran =
    problems.length === 0
      ? facts.runCheck(inspected.worktree, record.base)
      : { status: 1, output: "" };
  const passed = problems.length === 0 && ran.status === 0;
  const output = tail(
    [...problems, ran.output].filter((part) => part !== "").join("\n"),
    rules.outputLimit,
  );
  const base = {
    ...record,
    head: inspected.head,
    worktree: inspected.worktree,
    report: report,
    protectedChanges: inspected.protectedChanges ?? [],
    reportLines: reportLinesOf(
      input.last_assistant_message,
      record.prompt,
      rules,
    ),
    output,
    updatedAt: facts.now(),
  };
  const warning =
    facts.lock === LOCK.off && base.protectedChanges.length > 0
      ? `\n보호 경로 변경(잠금이 꺼져 있어 경고만 한다): ${base.protectedChanges.join(", ")}`
      : "";
  if (passed && report !== REPORT.invalid) {
    return {
      record: { ...base, status: STATUS.passed },
      output:
        warning === ""
          ? undefined
          : {
              systemMessage: `implementer ${input.agent_id}: ${warning.trim()}`,
            },
    };
  }
  if (
    !passed &&
    report === REPORT.blocked &&
    record.blocks >= rules.maxBlocks
  ) {
    return {
      record: { ...base, status: STATUS.blocked },
      output: {
        systemMessage: `implementer ${input.agent_id}: just check 실패 상태로 "막힘:" 종료 (${record.blocks}회 차단 뒤)`,
      },
    };
  }
  const blocks = record.blocks + 1;
  const last =
    blocks >= rules.maxBlocks
      ? '\n\n마지막 차단이다. 고칠 수 없으면 보고서 첫 줄을 "막힘:"으로 시작하고 남은 실패와 원인을 쓴 뒤 끝내라.'
      : "";
  const why = passed
    ? FIRST_LINE_RULE
    : `just base=${record.base} check 실패 (${blocks}/${rules.maxBlocks}). 검사를 끄거나 테스트를 약화하지 말고 원인을 고쳐라.${report === REPORT.invalid ? `\n${FIRST_LINE_RULE}` : ""}`;
  return {
    record: {
      ...base,
      blocks,
      status: passed ? STATUS.passed : STATUS.failing,
    },
    output: {
      decision: "block",
      reason: `${why}${warning}\n\n${output}${last}`,
    },
  };
};
