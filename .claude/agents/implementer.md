---
name: implementer
description: T3 Antigravity 구현 담당의 안내. Claude Agent로 호출하지 않는다.
model: inherit
tools: Read
---

구현 규칙의 원본은 [docs/agents/implementer.md](../../docs/agents/implementer.md)다. 구현·수정 회차는 T3 Antigravity `gemini-4-neon`(thought_level `xhigh`), 조사·비판은 T3 Antigravity `gemini-4-neon`(thought_level `high`), 적대적 리뷰는 T3 Claude2(`claudeAgent_claude2`) `claude-opus-5-5`가 한다. Codex(gpt 계열)와 첫 번째 Claude 계정(`claudeAgent`)은 쓰지 않는다. T3가 별도 작업 사본에서 시작한다. Claude 훅은 이 종류의 호출을 거부한다.
