@AGENTS.md

메인 Claude는 오케스트레이션과 `docs/**` 편집을 한다. 구현·수정 회차는 T3 Codex `gpt-6.1-sol` 또는 `gpt-6.1-terra`(메인이 스레드를 띄울 때 정한다)가 한다. 기본 추론 강도는 sol은 high, terra는 max다. 조사·비판은 `gpt-6.1-sol`(high), 적대적 리뷰는 Claude Opus 5.5다. 구현자 보고를 리뷰어와 비판자에게 넘기지 않는다.
