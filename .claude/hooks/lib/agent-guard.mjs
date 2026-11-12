import { BRIEF_PROBLEM, parseBrief } from "./brief.mjs";
import { lineDigest, meaningfulLines, normalized } from "./normalize.mjs";
import { ROLE, roleOf } from "./roles.mjs";

const finding = (rule, message) => ({ rule, gated: false, message });

const BRIEF_MESSAGES = new Map([
  [
    BRIEF_PROBLEM.noBase,
    "브리프에 `- 기준 커밋: <SHA>` 줄이 없다 (docs/workflow.md 2.1)",
  ],
  [BRIEF_PROBLEM.manyBases, "브리프에 서로 다른 기준 커밋이 여럿 있다"],
  [BRIEF_PROBLEM.badBase, "기준 커밋은 7~40자리 소문자 16진수 커밋 ID다"],
  [BRIEF_PROBLEM.manyBranches, "브리프에 서로 다른 브랜치가 여럿 있다"],
  [BRIEF_PROBLEM.badBranch, "브랜치 이름이 잘못됐다"],
]);

export const baseBranchName = (rules) =>
  rules.baseRef.replace(/^refs\/heads\//u, "");

const branchProblem = (brief, facts, rules) => {
  if (brief.branch === undefined) {
    return "브리프에 작업 브랜치가 없다: `- 기준 커밋: <SHA>, 브랜치 `<이름>``";
  }
  if (brief.branch === baseBranchName(rules)) {
    return `작업 브랜치는 기준 브랜치(${baseBranchName(rules)})가 아니어야 한다`;
  }
  return facts.worktreeForBranch(brief.branch) === undefined
    ? `브랜치 ${brief.branch}가 체크아웃된 작업 사본(원본 밖, git worktree에 등록된 것)이 없다. 먼저 git worktree add로 만든다`
    : undefined;
};

export const baseVerdict = (brief, facts, rules) => {
  if (brief.problem !== undefined) {
    return {
      problem: `${BRIEF_MESSAGES.get(brief.problem)}${brief.values === undefined ? "" : `: ${brief.values.join(", ")}`}`,
    };
  }
  const branch = branchProblem(brief, facts, rules);
  if (branch !== undefined) {
    return { problem: branch };
  }
  const commit = facts.resolveCommit(brief.commit);
  if (commit === undefined) {
    return {
      problem: `기준 커밋 ${brief.commit}을 하나의 커밋으로 풀 수 없다`,
    };
  }
  if (commit === facts.baseTip) {
    return { commit, source: "base-branch" };
  }
  const passed = facts.passedTargets.get(commit);
  if (passed !== undefined) {
    return { commit, source: `passed:${passed}` };
  }
  return {
    problem: `기준 커밋 ${commit}은 기준 브랜치 끝(${facts.baseTip ?? "없음"})도, 상태가 passed인 앞 단계의 대상 SHA도 아니다 (ADR 0013, 0018)`,
  };
};

const reportProblems = (prompt, rules, reportDigests) => {
  const lines = prompt.split("\n").map(normalized);
  const done = normalized(rules.reportFirstLines.done);
  const blocked = normalized(rules.reportFirstLines.blocked);
  const whole = normalized(prompt);
  const markers = [
    ...lines
      .filter((line) => line === done || line.startsWith(blocked))
      .map((line) => `보고 첫 줄 형태: ${line.slice(0, 40)}`),
    ...rules.reportMarkers
      .filter((marker) => whole.includes(normalized(marker)))
      .map((marker) => `보고 표식: ${marker}`),
    ...meaningfulLines(prompt, rules.reportLineMinimum)
      .filter((line) => reportDigests.has(lineDigest(line)))
      .map((line) => `구현자 보고의 줄: ${line.trim().slice(0, 60)}`),
  ];
  return markers.length === 0
    ? []
    : [
        finding(
          "implementer-report",
          `리뷰어·비판자에게 구현자의 보고를 넘기지 않는다 (docs/workflow.md 2.3, docs/harness.md 7.5): ${markers.join("; ")}`,
        ),
      ];
};

export const agentFindings = (input, rules, facts) => {
  if (rules.forbiddenTools.includes(input.tool_name)) {
    return [
      finding(
        "send-message",
        `${input.tool_name}로 끝난 서브에이전트를 이어 쓰지 않는다. 수정은 새 implementer에게 수정 브리프로 한다 (docs/harness.md 7.4)`,
      ),
    ];
  }
  if (roleOf(input) !== ROLE.main) {
    return [
      finding(
        "nested-agent",
        "서브에이전트는 서브에이전트를 띄우지 않는다 (docs/harness.md 7.4)",
      ),
    ];
  }
  const toolInput = input.tool_input ?? {};
  const type = toolInput.subagent_type;
  const extra = Object.keys(toolInput).filter(
    (key) => !rules.allowedArguments.includes(key),
  );
  const findings = [
    ...(type === undefined || type === null || type === "" || type === "fork"
      ? [
          finding(
            "fork",
            "포크와 종류 없는 호출은 쓰지 않는다 (docs/harness.md 7.4)",
          ),
        ]
      : rules.subagentTypes.includes(type)
        ? []
        : [
            finding(
              "agent-type",
              `허용되지 않은 서브에이전트 종류: ${String(type)} (허용: ${rules.subagentTypes.join(", ")})`,
            ),
          ]),
    ...(extra.length === 0
      ? []
      : [
          finding(
            "agent-argument",
            `인자 ${extra.join(", ")}는 쓰지 않는다. 모델·격리·실행 방식은 정의 파일이 정한다 (docs/harness.md 7.4)`,
          ),
        ]),
  ];
  const prompt = typeof toolInput.prompt === "string" ? toolInput.prompt : "";
  if (type === "implementer") {
    const verdict = baseVerdict(parseBrief(prompt), facts, rules);
    return verdict.problem === undefined
      ? findings
      : [...findings, finding("brief-base", verdict.problem)];
  }
  if (rules.subagentTypes.includes(type) && type !== "implementer") {
    return [...findings, ...reportProblems(prompt, rules, facts.reportDigests)];
  }
  return findings;
};
