# 진행 상황

작업 상태는 이 파일에 적는다. 작업 내용과 완료 기준은 [tasks.md](tasks.md)에 있다.

| 작업 | 상태 | 대상 커밋 | 리뷰 기록 | 뒤 작업에 넘긴 것 |
|---|---|---|---|---|
| H-01 | 완료 | 8dc8542 | [H-01](reviews/H-01.md) | U-03: CSP·`freezePrototype`·명시적 capability 목록. H-33: 빈 `capabilities` 목록을 명시로 보지 않기. H-02: `CARGO_TARGET_DIR` 고정. H-21: Tauri CLI 버전 고정과 `frontendDist` 교체. H-13 또는 S-14: 앱 묶음에 `gen-bindings`가 들어가는 문제 |
| H-19 | 완료 | 8dc8542 | [H-01](reviews/H-01.md) | - |
| H-02, H-03 | 완료 | 1bf0c1e | [H-02](reviews/H-02.md) | H-23·H-35·H-16: cargo 설정 파일과 `PATH` 우회. H-40: 끌 수 있는 린트는 그룹 소속만. H-05: `include!` |
| H-21, H-06 | 완료 | 71157a3 | [H-21](reviews/H-21.md) | H-17: `dist` 없는 사본. H-10: `packageManager` 해시 |
| H-04, H-22 | 완료 (3회차까지) | c138c2a | [H-04](reviews/H-04.md) | T-16: 범용 HTTP는 `broker-toss-trade`에만, `sources`의 토스 자격 증명 차단. RV-18 추가 |
| H-05, H-23, H-37 | 완료 (3회차까지) | c138c2a | [H-05](reviews/H-05.md) | H-40: 그룹 이름·`mod`/`impl`의 `expect`. H-18·H-17: `.cargo` 허용 열기. 시계 어댑터 작업: 허용 목록 추가 |
| H-07, H-24, H-10 | 완료 | 4a56501 | [H-07](reviews/H-07.md) | H-08: `.catch` 금지와 허용 목록, `///` 지시문, 대문자 확장자. U-03: CSP와 외부 이동 거부. DS-10: `ref`는 `RefObject`만. H-11: Vitest 건너뛰기 0건. H-38: `~~x`, `x \| 0` |
| H-31, H-32, H-33 | 완료 | a8b6a70 | [H-31](reviews/H-31.md) | S-20: SQL 검사 구조 바꾸기(파서, 금액 열을 스키마에서). H-13: 등록 명령·명령 목록·권한 묶음 대조, `build.rs` 본문 고정. S 묶음: 마이그레이션 잠금에 올리는 절차 |
| H-39, H-40(Rust) | 완료 | 7575e7c | [H-39](reviews/H-39.md) | 도메인 작업: `harness/raw-types/rules.toml`에 없는 표준 타입이 필요하면 승인으로 추가. H-40(TS): `disable-report`에 TS 끄기 더하기 |
| H-08, H-38 | 완료 | e9f0770 | [H-08](reviews/H-08.md) | DS-05: 표시 함수. DS-09·DS-10: JSX 속성 문구, DOM 접근. DS-18: 자유 입력 브랜드. U-04·U-05: IPC 호출 어댑터와 `detach`. 다국어: 보간 있는 문구 |
