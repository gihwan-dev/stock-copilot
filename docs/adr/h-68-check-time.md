# H-68: 검사 시간 줄이기

- 상태: 확정 (2026-11-25, fin-b 묶음, 사용자 결정 2026-11-24)
- 관련: ADR 0133(H-51), ADR 0173

## 결정

| 항목 | 정한 것 | 버린 대안 |
|---|---|---|
| 속성 테스트 횟수 | `check-fast`의 nextest는 `STOCK_COPILOT_PROPTEST_CASES`를 지운 채로 돌아 256회다(`testkit::property_cases` 기본값). `check`·`check-audited`·`coverage-check`·`linux-test`·CI는 레시피의 `[env(... "4096")]`를 그대로 둔다. `check-lite`는 이미 `env -u`로 256회다 | 사용자가 환경 변수를 직접 지우게 함 |
| 빌드 결과 공유 | 이번 묶음에서 바꾸지 않는다. H-51(ADR 0133)의 실측에서 메인 `target/` APFS 씨앗은 23% 느렸고, 공유 캐시(`sccache`)와 공유 `CARGO_TARGET_DIR`은 신뢰 경계와 빌드 잠금 때문에 사용자가 버렸다. 이 기기에서 다시 재는 일은 실측 행이라 건너뛴다. 공유가 없으므로 "묶음 끝에 `cargo clean`" 규칙을 그대로 둔다 | 공유 `CARGO_TARGET_DIR` 도입 |
| 변이 대상 | 대상 목록은 `.cargo/mutants.toml`의 `examine_globs` 한 곳이다. 변경분(`mutants-diff*`), 일간(`mutants-daily`), 주간 전체(`mutants`, `mutants-shard`)가 모두 `target_globs`로 이 목록을 읽는다. 목록을 금액·수량·환율·비율·확률 계산과 임계값·신호 판정(`domain` 파일 단위), 주문 판정·실행(`domain`의 주문·지정가·호가 단위 파일, `order-execution` 전체, `application`의 주문·신호 평가 유스케이스), 비밀·권한 경계(`application`의 비밀 포트·사용자 확인, `store-sqlite`의 암호화 연결·주문 속도 제한, `mcp-server`의 토큰·소켓·피어·비신뢰 입력)로 좁혔다 | 크레이트 전체 유지 |
| 목록 밖 코드 | 목록 밖 코드는 변이 검사를 돌리지 않고, 리뷰의 손 변형과 단위·속성 테스트로 갈음한다. AI 관련 작업은 ADR 0173대로 손 변형 20개 이상을 유지한다 | 목록 밖도 주간 전체로만 검사 |

## 측정

| 항목 | 전 | 후 |
|---|---:|---:|
| `cargo mutants --list` 변이 수(같은 커밋, 2026-11-25) | 11,332 | 2,616 |

변이 하나당 시간이 같다고 보면 변경분·일간·주간 변이 시간이 약 77% 준다. 실제 실행 시간(전·후)은 실측 항목이라 이번 묶음에서 재지 않았다. `check-fast`의 속성 횟수 변경 전·후 시간도 같은 이유로 재지 않았다.
