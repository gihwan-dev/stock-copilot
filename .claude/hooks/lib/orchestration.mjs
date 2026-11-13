import { agentFindings, baseVerdict } from "./agent-guard.mjs";
import { parseBrief } from "./brief.mjs";
import { ROLE, roleOf } from "./roles.mjs";

const denied = (message) => [{ rule: "orchestration", gated: false, message }];
export const orchestrationFindings = (input, rules, facts) => {
  if (roleOf(input) !== ROLE.main)
    return denied("서브에이전트는 위임하지 않는다");
  if (input.tool_name === "SendMessage")
    return denied("수정과 비판은 새 Codex 회차로 한다");
  if (input.tool_name === "Agent")
    return input.tool_input?.subagent_type === "adversarial-reviewer"
      ? agentFindings(input, rules, facts)
      : denied(
          "Claude 서브에이전트는 적대적 리뷰만 한다. 구현·수정·조사·비판은 T3 Codex gpt-6.1-sol이다",
        );
  const args = input.tool_input ?? {};
  if (
    args.target?.driverKind !== "codex" ||
    args.target?.model !== "gpt-6.1-sol" ||
    args.isolation !== "worktree"
  )
    return denied(
      "T3 위임은 driverKind=codex, model=gpt-6.1-sol, isolation=worktree로 고정한다",
    );
  if (typeof args.task !== "string") return denied("위임 브리프가 없다");
  const brief = parseBrief(args.task);
  const result = baseVerdict(
    brief,
    { ...facts, worktreeForBranch: () => "T3가 별도 작업 사본을 만든다" },
    rules,
  );
  return result.problem === undefined ? [] : denied(result.problem);
};
