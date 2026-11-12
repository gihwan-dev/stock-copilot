import { STATUS } from "./state.mjs";

export const EXPECTED_MODEL = "claude-opus-5-5";

const context = (text) => ({
  hookSpecificOutput: { hookEventName: "PostToolUse", additionalContext: text },
});

const modelNote = (model) =>
  model === EXPECTED_MODEL
    ? `모델=${model}`
    : `모델=${model ?? "알 수 없음"} (정의의 ${EXPECTED_MODEL}가 아니다)`;

export const agentResult = (input, readRecord, currentHead) => {
  const toolInput = input.tool_input ?? {};
  const response = input.tool_response ?? {};
  const type = toolInput.subagent_type;
  const id = response.agentId;
  if (response.status !== "completed") {
    return {
      output: context(
        `하네스 기록: ${type} ${id ?? "?"} 상태=${STATUS.unchecked} (도구 상태 ${response.status ?? "없음"}: 결과를 받지 못했다). 완료로 취급하지 않는다.`,
      ),
    };
  }
  if (type === "implementer") {
    const record = readRecord("implementer", id);
    const head =
      record?.worktree === undefined ? undefined : currentHead(record.worktree);
    const moved = record?.head !== undefined && head !== record.head;
    const status = moved
      ? STATUS.unchecked
      : (record?.status ?? STATUS.unchecked);
    const updated =
      record === undefined
        ? undefined
        : { ...record, resolvedModel: response.resolvedModel };
    const changes = record?.protectedChanges ?? [];
    return {
      record: updated,
      output: context(
        [
          `하네스 기록: implementer ${id} 상태=${status}`,
          `기준=${record?.base ?? "없음"}`,
          `HEAD=${record?.head ?? "없음"}`,
          `차단=${record?.blocks ?? 0}`,
          modelNote(response.resolvedModel),
          ...(changes.length === 0
            ? []
            : [`보호 경로 변경=${changes.join(",")}`]),
          ...(record?.reason === undefined ? [] : [`이유=${record.reason}`]),
          ...(moved
            ? [`검사 뒤 HEAD가 ${head ?? "알 수 없음"}로 바뀌었다`]
            : []),
          status === STATUS.passed
            ? "passed다."
            : "passed가 아니므로 완료로 취급하지 않는다 (docs/workflow.md 2.2, 2.7).",
        ].join(", "),
      ),
    };
  }
  const record = readRecord("reviewer", id);
  const status = record?.status ?? STATUS.unchecked;
  return {
    record:
      record === undefined
        ? undefined
        : { ...record, resolvedModel: response.resolvedModel },
    output: context(
      [
        `하네스 기록: ${type} ${id} 원본 상태=${status}`,
        modelNote(response.resolvedModel),
        status === "clean"
          ? "원본과 작업 사본이 시작 때와 같다."
          : "원본 저장소의 git status와 HEAD를 직접 확인하고, 다르면 다음 단계로 가지 않는다 (docs/workflow.md 2.3).",
      ].join(", "),
    ),
  };
};
