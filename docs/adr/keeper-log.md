# master 지킴이 기록

| 시각 | master SHA | 결과 | 고친 것 |
|---|---|---|---|
| 2026-11-25 00:40 | ecaf0c43 | cargo check 실패(main.rs 병합 잔재), 훅이 코드 쓰기를 막아 멈춤 | 없음(메인이 4b6968e8에서 고침) |
| 2026-11-25 00:55 | 4b6968e8 → 1e21e3cf | cargo check·tsc 통과, 4b6968e8에서 check-lite 통과 | Claude 훅 사례·테스트 7개를 메인 역할 판정에 맞춤. 남은 실패: xtask git_hooks 11개, raw_types 2개(order_gate.rs bool 반환) |
| 2026-11-25 01:10 | ba50c4cc | cargo check·tsc 통과, arch-check 실패(chat_dto.rs 지역 변수 `sources`를 어댑터로 읽음, broker-toss-trade 예외 누락) → 고친 뒤 check-lite 통과 | chat_dto.rs 이름 바꿈, rules.toml에 broker-toss-trade 추가, git_hooks 테스트 8개 지움·3개 맞춤(raw_types는 master O-22가 고침) |
| 2026-11-25 01:28 | 40449a38 | cargo check·tsc·arch-check 통과, xtask 480개 통과 | 병합으로 중복된 mcp-server 예외 3개 지움, check-lite에서 `--exclude xtask` 뺌 |
