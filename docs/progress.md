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
| H-11(Rust), H-29(Rust), H-17, H-18 | 완료 | e376b39 | [H-11](reviews/H-11.md) | H-14·H-25: 기준 SHA 전달과 낡은 빌드. H-35: 샌드박스 중첩. H-16: 밤 전체 변이, 깨끗한 빌드. H-29(TS): TS 테스트 파일의 삭제·약화 |
| H-09, H-11(Vitest) | 완료 | 30d397a | [H-09](reviews/H-09.md) | H-29(TS). DS 작업: `import.meta` 금지 재검토. 순환 검사는 `knip`으로 대체(타입 전용 순환은 못 잡음) |

## 검사 명령

- `just base=<HEAD 또는 40자리 커밋 ID> check`: 기준은 HEAD의 조상이어야 한다. 병합 확인은 병합 전 `master` 끝을 기준으로 돈다.
- `just base=<…> check-full`: 전체 변이 테스트와 권고 DB 갱신 포함.

| 작업 | 상태 | 대상 커밋 | 리뷰 기록 | 뒤 작업에 넘긴 것 |
|---|---|---|---|---|
| H-30, H-14, H-40(TS) | 완료 | 5e06d24 | [H-30](reviews/H-30.md) | H-15: git 우회 명령 거부, 하위 `.gitignore`. H-35: `node_modules`·`target` 쓰기. H-16: 깨끗한 checkout. H-20: 훅 설정 보호 |
| H-13 | 완료 | 3aa4e32 | [H-13](reviews/H-13.md) | U-02: 입력 DTO. U-04: 호출 시간 제한, knip 무시 항목 빼기. U-05: 이벤트 API 허용 파일. S-14: `gen-bindings` 묶음에서 빼기 |

## 훅

`lefthook install`을 실행했다(2026-11-12). 커밋과 병합 커밋마다 `just base=HEAD check`가 돈다(약 3분). 추적하지 않는 파일이나 스테이징하지 않은 변경이 있으면 커밋이 거부된다.

| 작업 | 상태 | 대상 커밋 | 리뷰 기록 | 뒤 작업에 넘긴 것 |
|---|---|---|---|---|
| H-15 | 완료 | b2fa2c3 | [H-15](reviews/H-15.md) | H-35: 샌드박스가 셸 훅의 빈틈(스크립트 실행, 인터프리터 쓰기, 하드 링크)을 막음. H-26: 서명 검사. H-20: 잠금 켜기와 훅 해시. H-29: `.mjs` 테스트 변경 검출 |

## Claude Code 훅

`.claude/settings.json`의 훅은 이 저장소에서 연 Claude Code 세션에만 적용된다. 잠금 스위치(`harness/claude-hooks/lock.txt`)는 `off`다. 꺼진 동안 보호 경로 편집은 경고만 나오고, 금지 셸 명령 거부와 편집 뒤 린트는 동작한다. 그 세션에서 구현은 `implementer` 종류의 서브에이전트만 할 수 있고(다른 종류는 편집 거부), 메인은 `docs/**`만 편집한다.

| 작업 | 상태 | 대상 커밋 | 리뷰 기록 | 뒤 작업에 넘긴 것 |
|---|---|---|---|---|
| H-25 | 완료 | 76fdc5d | [H-25](reviews/H-25.md) | H-26: 작업 사본의 보호 파일 변경, 상태 기록 무결성 |
| H-35 | 일부 완료 | 76fdc5d | [H-25](reviews/H-25.md) | 샌드박스 설정은 준비·실측했지만 켜지 않았다. H-20 전에 원본 루트에서 샌드박스 안 `just check`가 통과해야 한다 |

## 샌드박스

Claude Code 샌드박스는 아직 꺼져 있다. 복사본 실측에서 작업 사본의 검사와 커밋은 샌드박스 안에서 통과했지만, 원본 루트에서는 하네스 테스트가 임시 git 저장소를 만들다 실패했다. 켜질 때까지 에이전트의 셸은 홈의 자격 증명을 읽을 수 있다(막는 것은 셸 훅의 문자열 검사와 `Read` 도구 거부뿐).

| 작업 | 상태 | 대상 커밋 | 리뷰 기록 | 뒤 작업에 넘긴 것 |
|---|---|---|---|---|
| H-12, H-36 | 완료 | 8c745d2 | [H-12](reviews/H-12.md) | - |
| 샌드박스 안 검사 실패 (H-20 선행 조건) | 해결 | 8c745d2 | [H-12](reviews/H-12.md) | 실제 원본 위치(`~/Documents`)에서의 실측은 H-20에서 사용자가 확인 |

## 남은 하네스 작업

| 작업 | 누가 | 필요한 것 |
|---|---|---|
| H-28 서명 키 | 사용자 | Secretive SSH 키, `harness/allowed_signers` |
| H-16, H-26, H-27 CI | 에이전트 | 원격 저장소(GitHub 비공개 저장소)를 만들지 사용자 결정이 필요하다. 없으면 작업 흐름 파일만 만들고 검증하지 못한다 |
| H-20 잠금 | 사용자 (Claude Code 밖에서) | ADR 0018의 절차: `node harness/sandbox/prelock.mjs` 확인 → 잠금 설정 생성 → `lock.txt` 켜기 → 해시 다시 쓰기 → 서명 커밋 |
