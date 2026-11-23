import { baseVerdict, reportProblems } from "./agent-guard.mjs";
import { parseBrief, reviewTargetCommit } from "./brief.mjs";
import { ROLE, roleOf } from "./roles.mjs";
const denied = (message) => [{ rule: "orchestration", gated: false, message }];
const plainObject = (value) =>
  value !== null && typeof value === "object" && !Array.isArray(value);
const keys = (value, allowed) =>
  plainObject(value) &&
  Object.keys(value).every((key) => allowed.includes(key));
const exactKey = (value, key) =>
  plainObject(value) &&
  Object.keys(value).length === 1 &&
  Object.hasOwn(value, key);
const safeMode = (args) =>
  args.runtimeMode === undefined ||
  ["inherit", "approval-required"].includes(args.runtimeMode);
const antigravityThoughtLevel = (role) =>
  role === "implementation"
    ? "xhigh"
    : role === "research" || role === "review"
      ? "high"
      : undefined;
const antigravityOptions = (value, role) => {
  const expected = antigravityThoughtLevel(role);
  return (
    expected !== undefined &&
    exactKey(value, "thought_level") &&
    value.thought_level === expected
  );
};
const claudeReviewOptions = (value) =>
  value === undefined || (exactKey(value, "effort") && value.effort === "high");
const antigravityFindings = (model, role, value, instanceId) => {
  if (instanceId !== undefined && instanceId !== "antigravity")
    return denied(
      "Antigravity 제공자 인스턴스는 생략 또는 antigravity만 허용한다",
    );
  const expected = antigravityThoughtLevel(role);
  if (model !== "gemini-4-neon" || expected === undefined)
    return denied(
      "Antigravity 허용 모델은 gemini-4-neon이며 역할은 implementation(xhigh), research·review(high)만 허용한다",
    );
  return antigravityOptions(value, role)
    ? []
    : denied(
        `Antigravity gemini-4-neon의 ${role} 역할은 thought_level=${expected} 옵션만 명시적으로 허용한다. effort·reasoningEffort 등 다른 키와 혼합은 거부한다`,
      );
};
const CLAUDE_INSTANCE_PATTERN = /^claudeAgent(?:$|[._-][A-Za-z0-9._-]+)$/u;
const claudeReviewFindings = (model, role, value, instanceId) => {
  if (
    instanceId !== undefined &&
    (typeof instanceId !== "string" ||
      !CLAUDE_INSTANCE_PATTERN.test(instanceId))
  )
    return denied(
      "적대적 리뷰의 providerInstanceId는 생략하거나 claudeAgent 형식의 Claude 인스턴스 이름이어야 한다",
    );
  if (model !== "claude-opus-5-5" || role !== "review")
    return denied(
      "Claude 적대적 리뷰 허용 대상은 driverKind=claudeAgent, model=claude-opus-5-5의 review 역할만 허용한다",
    );
  return claudeReviewOptions(value)
    ? []
    : denied(
        "Claude claude-opus-5-5 리뷰는 옵션 생략 또는 effort=high만 허용한다. 다른 키와 혼합은 거부한다",
      );
};
const launchRole = (message) => {
  if (typeof message !== "string") return undefined;
  const roles = [...message.matchAll(/^[-] 역할: (.+)$/gmu)];
  return roles.length === 1 ? roles[0][1] : undefined;
};
export const delegateTargetFindings = (target, role) => {
  if (!keys(target, ["driverKind", "model", "options", "providerInstanceId"]))
    return denied("허용하지 않은 T3 위임 인자다");
  if (target.driverKind === "codex")
    return denied(
      "Codex 제공자는 더 이상 허용하지 않는다. 구현·조사·비판은 antigravity(gemini-4-neon), 적대적 리뷰는 claudeAgent(claude-opus-5-5)만 사용한다",
    );
  if (target.driverKind === "antigravity")
    return antigravityFindings(
      target.model,
      role,
      target.options,
      target.providerInstanceId,
    );
  if (target.driverKind === "claudeAgent")
    return claudeReviewFindings(
      target.model,
      role,
      target.options,
      target.providerInstanceId,
    );
  return denied("허용하지 않은 T3 위임 인자다");
};
const launchSelectionFindings = (selection, message) => {
  if (!keys(selection, ["provider", "instanceId", "model", "options"]))
    return denied("현재 프로젝트를 상속하는 Antigravity worktree만 시작한다");
  if (selection.provider === "codex")
    return denied(
      "Codex 제공자는 더 이상 허용하지 않는다. 구현 스레드는 antigravity의 gemini-4-neon(thought_level=xhigh)만 시작한다",
    );
  if (selection.provider !== "antigravity")
    return denied("현재 프로젝트를 상속하는 Antigravity worktree만 시작한다");
  if (
    selection.instanceId !== undefined &&
    selection.instanceId !== "antigravity"
  )
    return denied(
      "Antigravity 제공자 인스턴스는 생략 또는 antigravity만 허용한다",
    );
  if (
    selection.model !== "gemini-4-neon" ||
    launchRole(message) !== "implementation"
  )
    return denied(
      "Antigravity 구현 스레드는 gemini-4-neon 모델과 정확히 한 개의 '- 역할: implementation' 줄만 허용한다",
    );
  return antigravityOptions(selection.options, "implementation")
    ? []
    : denied(
        "Antigravity gemini-4-neon 구현 스레드는 thought_level=xhigh 옵션만 명시적으로 허용한다. effort·reasoningEffort 등 다른 키와 혼합은 거부한다",
      );
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
    const targetProblems = delegateTargetFindings(args.target, args.role);
    if (targetProblems.length > 0) return targetProblems;
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
      args.isolation !== "worktree" ||
      (args.mode !== undefined && !["wait", "async"].includes(args.mode)) ||
      typeof args.task !== "string"
    )
      return denied("허용하지 않은 T3 위임 인자다");
    return args.role === "review"
      ? reviewBriefCheck(args.task, rules, facts)
      : briefCheck(args.task, rules, facts);
  }
  if (tool === "t3_thread_launch") {
    const selectionProblems = launchSelectionFindings(
      args.modelSelection,
      args.message,
    );
    if (selectionProblems.length > 0) return selectionProblems;
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
      !keys(args.workspaceStrategy, [
        "type",
        "branch",
        "baseRef",
        "startFromOrigin",
      ]) ||
      args.workspaceStrategy.type !== "worktree" ||
      typeof args.message !== "string"
    )
      return denied("현재 프로젝트를 상속하는 Antigravity worktree만 시작한다");
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
