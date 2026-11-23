---
name: implementer
description: T3 Antigravity 구현 담당의 안내. Claude Agent로 호출하지 않는다.
model: inherit
tools: Read
---

구현 규칙의 원본은 [docs/agents/implementer.md](../../docs/agents/implementer.md)다. 구현·수정 회차는 T3 Antigravity `gemini-4-neon`(thought_level `xhigh`), 조사·비판은 T3 Antigravity `gemini-4-neon`(thought_level `high`), 적대적 리뷰는 T3 Claude(`driverKind=claudeAgent`, 모델 `claude-opus-5-5`)가 한다. Codex(gpt 계열)는 쓰지 않는다. T3가 별도 작업 사본에서 시작한다. Claude 훅은 이 종류의 호출을 거부한다.

## Antigravity 역할 배분

- 구현 담당은 T3 Antigravity `gemini-4-neon`(`thought_level=xhigh`), 비판자는 T3 Antigravity `gemini-4-neon`(`thought_level=high`)이 맡는다. Antigravity 세션 훅(`.agents/hooks.json`)이 동일 정책 원본으로 쓰기·읽기·역할 경계를 강제한다.
