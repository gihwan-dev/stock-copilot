---
name: implementer
description: T3 Codex 구현 담당의 안내. Claude Agent로 호출하지 않는다.
model: inherit
tools: Read
---

구현 규칙의 원본은 [docs/agents/implementer.md](../../docs/agents/implementer.md)다. 구현·수정 회차는 T3 Codex `gpt-6.1-sol` 또는 `gpt-6.1-terra`(메인이 스레드를 띄울 때 정한다)가 한다. 기본 추론 강도는 sol은 high, terra는 max다. 조사·비판은 `gpt-6.1-sol`(high), 적대적 리뷰는 Claude Opus 5.5다. T3가 별도 작업 사본에서 시작한다. Claude 훅은 이 종류의 호출을 거부한다.
