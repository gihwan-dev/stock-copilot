import { agentFindings, baseVerdict } from "./agent-guard.mjs";
import { parseBrief } from "./brief.mjs";
import { ROLE, roleOf } from "./roles.mjs";
const denied = (message) => [{ rule: "orchestration", gated: false, message }];
const keys = (value, allowed) =>
  value !== null &&
  typeof value === "object" &&
  Object.keys(value).every((key) => allowed.includes(key));
const safeMode = (args) =>
  args.runtimeMode === undefined ||
  ["inherit", "approval-required"].includes(args.runtimeMode);
const options = (value) =>
  value === undefined ||
  (keys(value, ["effort"]) &&
    (value.effort === undefined || value.effort === "high"));
const briefCheck = (text, rules, facts) => {
  const result = baseVerdict(
    parseBrief(text),
    { ...facts, worktreeForBranch: () => "T3 worktree" },
    rules,
  );
  return result.problem === undefined ? [] : denied(result.problem);
};
export const orchestrationFindings = (input, rules, facts) => {
  if (roleOf(input) !== ROLE.main)
    return denied("서브에이전트는 위임하지 않는다");
  if (input.tool_name === "SendMessage")
    return denied("Claude 리뷰어를 재개하지 않는다");
  if (input.tool_name === "Agent")
    return input.tool_input?.subagent_type === "adversarial-reviewer"
      ? agentFindings(input, rules, facts)
      : denied("Claude는 매번 새 적대적 리뷰만 한다");
  const tool = input.tool_name.replace(/^mcp__t3[-_]code__/u, "");
  const args = input.tool_input ?? {};
  if (
    !safeMode(args) ||
    (args.interactionMode !== undefined &&
      !["default", "plan"].includes(args.interactionMode))
  )
    return denied("권한 확대는 허용하지 않는다");
  if (tool === "delegate_task") {
    if (
      !keys(args, [
        "task",
        "title",
        "target",
        "isolation",
        "mode",
        "role",
        "runtimeMode",
        "interactionMode",
        "clientRequestId",
        "timeoutMs",
      ]) ||
      !keys(args.target, ["driverKind", "model", "options"]) ||
      args.target.driverKind !== "codex" ||
      args.target.model !== "gpt-6.1-sol" ||
      !options(args.target.options) ||
      args.isolation !== "worktree" ||
      (args.role !== undefined &&
        !["implementation", "research", "review"].includes(args.role)) ||
      (args.mode !== undefined && !["wait", "async"].includes(args.mode)) ||
      typeof args.task !== "string"
    )
      return denied("허용하지 않은 T3 위임 인자다");
    return briefCheck(args.task, rules, facts);
  }
  if (tool === "t3_thread_launch") {
    if (
      !keys(args, [
        "title",
        "message",
        "modelSelection",
        "workspaceStrategy",
        "runtimeMode",
        "interactionMode",
        "clientRequestId",
      ]) ||
      !keys(args.modelSelection, ["provider", "model", "options"]) ||
      args.modelSelection.provider !== "codex" ||
      args.modelSelection.model !== "gpt-6.1-sol" ||
      !options(args.modelSelection.options) ||
      !keys(args.workspaceStrategy, [
        "type",
        "branch",
        "baseRef",
        "startFromOrigin",
      ]) ||
      args.workspaceStrategy.type !== "worktree" ||
      typeof args.message !== "string"
    )
      return denied("현재 프로젝트를 상속하는 Codex worktree만 시작한다");
    const brief = parseBrief(args.message);
    const strategy = args.workspaceStrategy;
    if (typeof strategy.branch !== "string" || strategy.branch !== brief.branch)
      return denied("작업 사본의 브랜치와 브리프가 다르다");
    if (strategy.startFromOrigin === true) {
      const origin = facts.originTip ?? facts.resolveCommit("origin/master");
      if (
        strategy.baseRef !== "origin/master" ||
        !origin ||
        facts.resolveCommit(brief.commit) !== origin
      )
        return denied("원격 시작은 origin/master 끝과 같은 브리프만 쓴다");
      return briefCheck(args.message, rules, { ...facts, baseTip: origin });
    }
    if (strategy.startFromOrigin !== false)
      return denied("로컬·원격 기준 선택을 명시한다");
    if (args.workspaceStrategy.baseRef !== brief.commit)
      return denied("작업 사본의 기준과 브리프가 다르다");
    return briefCheck(args.message, rules, facts);
  }
  const readers = {
    t3_project_list: [],
    orchestrator_capabilities: [],
    t3_background_wait: ["timeoutMs", "untilDelegatedWorkSettled"],
  };
  if (tool in readers)
    return keys(args, readers[tool])
      ? []
      : denied("읽기·대기 도구의 허용 인자 밖이다");
  const allowed = {
    t3_thread_send: ["threadId", "message", "mode", "clientRequestId"],
    t3_thread_read: [
      "threadId",
      "view",
      "limit",
      "maxCharsPerItem",
      "afterPosition",
      "itemId",
      "textOffset",
      "runLimit",
    ],
    t3_thread_wait: [
      "threadId",
      "runId",
      "timeoutMs",
      "untilDelegatedWorkSettled",
    ],
  };
  if (!(tool in allowed) || !keys(args, allowed[tool]))
    return denied("허용하지 않은 스레드 도구나 인자다");
  const owned = facts.t3Threads?.get(args.threadId);
  const worktree =
    owned === undefined ? undefined : facts.worktreeForBranch(owned.branch);
  if (worktree === undefined)
    return denied("이 메인이 시작한 같은 저장소의 스레드가 아니다");
  if (tool === "t3_thread_send") {
    if (
      typeof args.message !== "string" ||
      (args.mode !== undefined && args.mode !== "auto")
    )
      return denied("수정 메시지 인자가 잘못됐다");
    const brief = parseBrief(args.message);
    const commit = facts.resolveCommit(brief.commit);
    if (
      brief.problem ||
      brief.branch !== owned.branch ||
      !commit ||
      !facts.isAncestor(facts.baseTip, commit) ||
      !facts.isAncestor(commit, facts.headOf(worktree))
    )
      return denied("수정 기준이 해당 작업 사본 이력에 없다");
  }
  return [];
};
export const threadResult = (response) => {
  if (response?.structuredContent !== undefined)
    return response.structuredContent;
  const text = response?.content?.find((item) => item.type === "text")?.text;
  if (typeof text === "string") {
    try {
      return JSON.parse(text);
    } catch {
      return undefined;
    }
  }
  return response;
};
