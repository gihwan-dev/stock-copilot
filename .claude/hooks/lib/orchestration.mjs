import { baseVerdict, reportProblems } from "./agent-guard.mjs";
import { parseBrief, reviewTargetCommit } from "./brief.mjs";
import { ROLE, roleOf } from "./roles.mjs";
const denied = (message) => [{ rule: "orchestration", gated: false, message }];
const keys = (value, allowed) =>
  value !== null &&
  typeof value === "object" &&
  Object.keys(value).every((key) => allowed.includes(key));
const safeMode = (args) =>
  args.runtimeMode === undefined ||
  ["inherit", "approval-required"].includes(args.runtimeMode);
const expectedEffort = (model) => (model === "gpt-6.1-terra" ? "max" : "high");
const options = (value, model) =>
  (model !== "gpt-6.1-terra" ||
    value?.effort === "max" ||
    value?.reasoningEffort === "max") &&
  (value === undefined ||
    (keys(value, ["effort"]) &&
      (value.effort === undefined || value.effort === expectedEffort(model))) ||
    (["gpt-6.1-sol", "gpt-6.1-terra"].includes(model) &&
      keys(value, ["reasoningEffort"]) &&
      value.reasoningEffort === expectedEffort(model)));
const codexModel = (model, role) =>
  model === "gpt-6.1-sol" ||
  (role === "implementation" && model === "gpt-6.1-terra");
const codexFindings = (model, role, value) => {
  if (!codexModel(model, role))
    return denied(
      "Codex 허용 모델은 gpt-6.1-sol 또는 gpt-6.1-terra이며 역할은 terra에 implementation을 명시한다. 조사·비판은 sol만 허용한다",
    );
  return options(value, model)
    ? []
    : denied(
        "Codex 추론 강도는 sol high, terra max이며 Terra의 max 옵션을 명시한다. effort와 reasoningEffort는 혼합하지 않고 한 키만 쓴다",
      );
};
const launchRole = (message) => {
  const roles = [...String(message).matchAll(/^[-] 역할: (.+)$/gmu)];
  return roles.length === 1 ? roles[0][1] : undefined;
};
const briefCheck = (text, rules, facts) => {
  const result = baseVerdict(
    parseBrief(text),
    { ...facts, worktreeForBranch: () => "T3 worktree" },
    rules,
  );
  return result.problem === undefined ? [] : denied(result.problem);
};
const reviewBriefCheck = (text, rules, facts) => {
  const brief = parseBrief(text);
  const base =
    brief.problem === undefined ? facts.resolveCommit(brief.commit) : undefined;
  const target =
    brief.branch === undefined
      ? undefined
      : facts.worktreeForBranch?.(brief.branch);
  const head = target === undefined ? undefined : facts.headOf?.(target);
  const requested = reviewTargetCommit(text);
  const targetCommit =
    requested === undefined ? undefined : facts.resolveCommit(requested);
  return base &&
    head &&
    base !== targetCommit &&
    targetCommit === head &&
    facts.isAncestor?.(base, targetCommit)
    ? reportProblems(text, rules, facts.reportDigests ?? new Set())
    : denied("리뷰 기준은 등록된 대상 작업 사본 이력의 커밋이어야 한다");
};
export const orchestrationFindings = (input, rules, facts) => {
  if (roleOf(input) !== ROLE.main)
    return denied("서브에이전트는 위임하지 않는다");
  if (input.tool_name === "SendMessage")
    return denied("Claude 리뷰어를 재개하지 않는다");
  if (input.tool_name === "Agent")
    return denied("리뷰도 T3의 별도 작업 사본으로만 위임한다");
  const tool = input.tool_name.replace(/^mcp__t3[-_]code__/u, "");
  const args = input.tool_input ?? {};
  if (
    !safeMode(args) ||
    (args.interactionMode !== undefined &&
      !["default", "plan"].includes(args.interactionMode))
  )
    return denied("권한 확대는 허용하지 않는다");
  if (tool === "delegate_task") {
    if (args.target?.driverKind === "codex") {
      const problems = codexFindings(
        args.target.model,
        args.role,
        args.target.options,
      );
      if (problems.length > 0) return problems;
    }
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
      !keys(args.target, [
        "driverKind",
        "model",
        "options",
        "providerInstanceId",
      ]) ||
      !(
        (args.target.driverKind === "codex" &&
          codexModel(args.target.model, args.role) &&
          (args.target.providerInstanceId === undefined ||
            args.target.providerInstanceId === "codex")) ||
        (args.target.driverKind === "claudeAgent" &&
          args.target.model === "claude-opus-5-5" &&
          args.role === "review" &&
          (args.target.providerInstanceId === undefined ||
            args.target.providerInstanceId === "claudeAgent"))
      ) ||
      !options(args.target.options, args.target.model) ||
      args.isolation !== "worktree" ||
      (args.role !== undefined &&
        !["implementation", "research", "review"].includes(args.role)) ||
      (args.mode !== undefined && !["wait", "async"].includes(args.mode)) ||
      typeof args.task !== "string"
    )
      return denied("허용하지 않은 T3 위임 인자다");
    return args.role === "review"
      ? reviewBriefCheck(args.task, rules, facts)
      : briefCheck(args.task, rules, facts);
  }
  if (tool === "t3_thread_launch") {
    if (args.modelSelection?.provider === "codex") {
      const problems = codexFindings(
        args.modelSelection.model,
        launchRole(args.message),
        args.modelSelection.options,
      );
      if (problems.length > 0) return problems;
    }
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
      !codexModel(args.modelSelection.model, launchRole(args.message)) ||
      !options(args.modelSelection.options, args.modelSelection.model) ||
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
    if (!["implementation", "research"].includes(owned.role))
      return denied("리뷰·비판은 재개하지 않고 새로 위임한다");
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
