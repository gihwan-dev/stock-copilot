@AGENTS.md

메인 Claude는 오케스트레이션과 `docs/**` 편집을 한다. 구현·수정 회차는 T3 Antigravity `gemini-4-neon`(thought_level `xhigh`), 조사·비판은 T3 Antigravity `gemini-4-neon`(thought_level `high`), 적대적 리뷰는 T3 Claude(`driverKind=claudeAgent`, 모델 `claude-opus-5-5`)가 한다. Codex(gpt 계열)는 쓰지 않는다. 구현자 보고를 리뷰어와 비판자에게 넘기지 않는다.
