# H-42: 파일 단위 변경분 변이와 제한된 병렬 실행

- 상태: 초안, 메인 평가 대기
- 기준: af70b46fc6c62a0d1a7b036c9977b3d2f3d47366
- 대체: ADR 0013 결정 표의 변이 범위와 `check-full` 전체 변이 중복 실행 결정. 테스트 보호, 대상 글롭, 생존 허용 목록과 시간 초과 실패는 유지한다.

## 이유

ADR 0013과 H-11 리뷰 9번은 테스트만 약화하면 변경 줄 변이가 0개가 되는 재현을 막기 위해 crate 전체를 선택했다. 당시 전제였던 빈 crate가 이제 1,260개 이상의 변이를 가진다. 파일 안의 `cfg(test)`만 바뀌어도 그 파일의 모든 변이를 실행하면 그 재현을 막으면서 보통 변경의 비용을 줄일 수 있다. cargo-mutants 자체 `--in-diff`는 쓰지 않는다.

## 범위 규칙

기준 객체와 현재 작업 사본을 `git diff --name-only -z --no-renames`로 비교하고 추적하지 않은 파일도 합친다. 기준은 기존 조상·40자리 SHA/HEAD 검증을 유지한다. `--file`은 설정과 합집합이므로 기존 `--no-config`를 유지한다.

| 변경 또는 판정 | 실행 범위 |
|---|---|
| 대상 글롭의 정상 Rust 파일 | 변경 파일 전체. 테스트 줄만 바뀌어도 동일 |
| 직접 참조 | 같은 crate Rust AST에서 변경 모듈 이름을 `use`, `mod`, 여러 세그먼트 경로로 참조하는 파일을 추가. 대상 글롭과 교집합 |
| crate의 `tests` 경로·이름에 test가 든 지원 파일·파일 내부 속성 | 그 crate의 대상 전체 |
| 속성이 붙은 외부 `mod helper;`로 선언한 지원 모듈 또는 그 하위 파일 | 그 crate의 대상 전체. 파일 이름 목록이 아니라 모듈 선언 구조로 판정 |
| crate `Cargo.toml`, Rust 외 지원 파일 | 그 crate의 대상 전체 |
| `lib.rs`의 공개 재수출 외 변경 | 그 crate의 대상 전체 |
| `lib.rs`의 속성 없는 `pub use`만 변경 | 그 파일과 직접 참조. 재수출을 제외한 AST 토큰이 같아야 함 |
| 삭제·이름 변경 | 이전 경로 읽기가 실패하므로 그 crate 전체. Git 이름 변경 추론에 의존하지 않음 |
| 읽기·목록·Rust 파싱 실패, 정확한 파일 필터로 쓸 수 없는 glob 특수문자 경로 | 그 crate 전체 |
| 루트 Cargo.toml/Cargo.lock/rust-toolchain.toml, mutants/nextest 설정 | 모든 대상 |
| 대상 crate 밖 또는 부분 대상 crate의 대상 밖 정상 `.rs` | 없음. 별도 테스트/지원 파일 규칙은 우선 적용 |

참조는 이름 기반으로 보수적으로 추가한다. 다른 모듈에서 같은 이름을 쓰면 불필요한 파일이 더 선택될 수 있다. `exact_arithmetic`처럼 완전 경로로 호출하는 공용 계산도 포함한다. 직접 참조만 확장하고 전이 폐쇄는 계산하지 않는다. `lib.rs`의 `mod money;`도 선택되므로 money 변경은 money와 lib 두 파일이 선택된다. 실행 전 실제 파일 목록과 jobs·시간 배수를 출력한다.

## 병렬 실측과 제한

macOS Apple Silicon, 논리 코어 16개, cargo-mutants 27.1.0. 다른 작업 사본들의 cargo-mutants·nextest·llvm-cov·rustc 동시 실행을 프로세스 목록으로 확인했다. 사용자 제시 5개 에이전트의 활동 수 자체를 따로 증명하지는 않았다. 같은 money.rs, 같은 33개 변이, 같은 옵션, 순서 2→4→6, 실행마다 baseline 포함. 한 번씩 측정했으므로 장시간 안정성의 보증은 아니다.

| jobs | 전체 경과 초 | caught | unviable | missed | timeout | 종료 |
|---|---:|---:|---:|---:|---:|---:|
| 2 | 189.21 | 20 | 13 | 0 | 0 | 0 |
| 4 | 160.02 | 20 | 13 | 0 | 0 | 0 |
| 6 | 127.66 | 20 | 13 | 0 | 0 | 0 |

기본값은 `min(max(floor(available_parallelism / 2), 1), 6)`이다. 이 기계는 6. `STOCK_COPILOT_MUTANTS_JOBS`는 1부터 기본값까지 낮추는 값만 허용한다. 0·음수·비정수·범위 초과·정수 오버플로·비 UTF-8은 실패다. 새 의존성은 없다.

공식 [병렬 실행 문서](https://mutants.rs/parallelism.html)는 낮은 jobs에서 시작하고 CPU·RAM·디스크 경합과 거짓 시간 초과를 경고한다. 단순 코어 비례가 권고는 아니므로 측정한 6을 상한으로 둔다. [jobserver 문서](https://mutants.rs/jobserver.html)의 기본 공유 CPU 제한을 유지한다. [시간 제한 문서](https://mutants.rs/timeouts.html)와 로컬 27.1.0 소스의 기본 테스트 배수 5를 근거로 `--timeout-multiplier=5*jobs`를 명시한다. baseline은 계속 실행한다. 실측에서도 각 jobs에 같은 공식을 적용했다. 무한 실행은 여전히 유한 시간 후 실패하며 시간 초과를 caught나 통과로 바꾸지 않는다. 실제 `cargo mutants --help`에서 `--jobs`, `--file`, `--in-diff`, `--timeout-multiplier`, jobserver 옵션을 확인했다.

[Nextest 문서](https://mutants.rs/nextest.html)대로 nextest를 유지한다. 저장소 `.config/nextest.toml` wrapper와 sandbox-exec 프로필은 수정하지 않는다. 세 병렬 실행 모두 wrapper 활성화와 전체 domain baseline 성공을 확인했다. 테스트마다 프로세스가 따로여서 동일 프로필 적용이 서로 덮어쓰지 않는다. 네트워크 차단 자체의 회귀는 기존 workspace 테스트로 확인한다.

## 전체 검사와 실패 처리

`check-full`은 `check` 뒤 `mutants-diff`만 실행한다. `.github/workflows/nightly.yml`의 Linux 작업은 180분 제한 안에서 `node harness/ci/linux/offline.mjs just mutants-all`을 실행한다. continue-on-error가 없다. Linux 실패는 `needs: [changes, macos, linux]`, `if: failure()`의 notify로 이어지고 보호된 알림 스크립트가 CI 실패 이슈를 만든다. workflow-policy가 기대 워크플로와 일치함을 검사한다. 워크플로는 변경하지 않았다. 실제 원격 실행·이슈 생성은 이번에 실행하지 않았다.

밤 전체 검사가 생존·시간 초과로 실패하면 CI 실패 이슈와 해당 run 로그를 확인하고 원인을 수정한 뒤 전체 검사를 다시 통과시킨다. 시간 초과를 무시하거나 생존 목록을 추가해서 닫지 않는다. 알림 도착까지의 시간과 원격 러너 성능은 이번 로컬 실측으로 보장하지 않는다. 180분을 넘으면 역시 작업 실패와 알림 대상이며, 장기적으로 이를 넘으면 메인이 별도 CI 용량 결정을 해야 한다.

## 남는 구멍

파일 범위는 변경 파일이 다른 파일의 테스트 의미를 바꾸는 모든 경로를 증명하지 못한다. 루트 재수출을 통한 타입/함수 사용, 별칭·매크로·문자열 생성 참조, 전이 참조, 다른 crate의 구현이나 테스트 도우미 변경은 직접 참조 확장에 없을 수 있다. 이름/속성/외부 모듈로 드러나지 않는 테스트 도우미도 놓칠 수 있다. 이런 경우 밤 전체 변이가 대상 안의 생존을 찾아야 한다. 밤 검사까지 탐지가 늦어지며, 대상 밖 코드와 cargo-mutants가 생성하지 않는 변이는 밤 검사도 증명하지 못한다. 기존 test-diff·커버리지·주문 경로 고정 검사는 독립적으로 유지한다.

## 검증 기록

시작 전체 check 통과, 구현 후 검사와 효과 실측 결과는 아래에 보충한다. `test_runner_tests.rs`의 기존 `check_full_mutates_every_target_crate` 하나를 `check_full_does_not_repeat_nightly_mutation_testing`으로 바꾸고 mutants-all 부재를 단언했다. check-full 중복 실행 제거 결정의 직접 결과다. 나머지 기존 테스트는 바꾸지 않았다. 새 scope_tests에서 정상 파일·인라인 테스트·작업 사본·커밋·지원 파일·매니페스트·삭제·이름 변경·직접 참조·부분 대상·파싱 실패·jobs 범위·check-full 중복 금지를 검사한다.

효과 실측: 이 브랜치에서 money.rs 테스트 생성 범위의 `28_u32` 한 곳을 같은 값 `0x1c_u32`로 임시 변경했다. `just mutants-diff af70b46fc6c62a0d1a7b036c9977b3d2f3d47366`은 2파일(money.rs, lib.rs), 33변이, 112.50초, caught 20·unviable 13·missed/timeout 0, 종료 0이었다. 바이트 원본을 finally로 복구했고 git diff가 비었음을 확인했다. 사용자가 제시한 이전 관측은 약 1,260개다. 이 브랜치에서 구방식 domain 전체를 `cargo mutants --no-config --file 'crates/domain/src/**/*.rs' --list`로 다시 열거하면 정확히 1,200개이고, 33개로 97.25% 줄었다. 이전 1~2시간은 사용자 제시 관측값이며 이번에 이전 전체 실행을 반복하지 않았다. 서로 다른 실행 부하이므로 시간 감소율을 정밀 비교하지 않는다.

로그는 작업 사본 `target/h42/`의 jobs-{2,4,6}.log 및 .timing.json, effect.log/effect.timing.json, initial check는 `target/h42/initial-check.log`에 있다. 결과 JSON을 진행 도중 최종값으로 간주하지 않고 cargo-mutants 프로세스 종료와 요약을 확인했다.

손 변형 10개를 각각 적용하고 새 범위 테스트를 실행했다. 빈 선택 반환, 직접 참조 확장 제거, 테스트 지원 확장 제거, lib 비재수출 확장 제거, 외부 테스트 모듈 확장 제거, 전역 입력 확장 제거, 판정 실패의 crate 확장 제거, 작업 사본 변경 무시, 대상 글롭 필터 제거, 비 Rust 지원 파일 확장 제거 모두 컴파일 후 실제 테스트 실패(종료 100)로 검출했다. 최초 시도 중 죽은 코드/미사용 import 때문에 컴파일에서 막힌 3개는 함수 호출을 보존한 손 변형으로 다시 실행해 실제 테스트 실패를 확인했다. 모든 손 변형을 원복했다. 로그/판정은 `target/h42/manual*.json`, `manual-*.log`에 있다.

justfile의 보호 기대값도 dumpJustfile로 재생성했다. 변경은 check-full 본문의 mutants-all 한 항목 제거뿐이다. 워크플로 기대값은 바꾸지 않았다.

환경 변수 읽기는 `mutants_check/environment.rs` 경계 한 파일에만 둔다. 기존 환경 접근 금지 검사 때문에 `harness/source-checks/allowlists.toml`에 그 파일의 `clock-env:var`만 추가했다. 제품과 나머지 xtask 파일의 환경 접근 금지는 유지한다. 환경 변수 하나를 읽는 요청을 구현하기 위한 필수 허용 항목이며 생존·시간 초과·대상 글롭 정책과 무관하다.

이 경계 추가에 따라 `source_check/tests.rs`의 환경 경계 경로 집합 기대값에도 새 경계 경로 하나를 더했다. 기존 허용 경로를 제거하거나 전역 환경 금지를 끄지 않았다. 이 기존 테스트 기대값 변경도 test-diff의 사용자 서명 반영 대상이다.


최종 검증: `just base=af70b46fc6c62a0d1a7b036c9977b3d2f3d47366 check`는 Rust workspace 513/513, 커버리지 실행 256/256, coverage-check·형식·Clippy·TS·구조·생성물 검사까지 통과했다. 마지막 test-diff가 기존 테스트 변경 3건(기존 check-full 테스트 이름의 removed/not-listed, 환경 경계 집합 테스트 modified)을 보고하여 종료 1이다. 검사 기준을 바꾸거나 훅을 우회하지 않았고 커밋은 만들지 않았다. 새 끄기 0건, hooks-installed 통과. 최종 로그는 target/h42/final-check3.log다. 메인이 사용자 서명으로 기존 테스트 변경을 반영한 뒤 승인된 새 기준으로 전체 check를 통과시키기 전에는 완료로 취급하지 않는다.

변경 파일: crates/xtask/src/mutants_check.rs, crates/xtask/src/mutants_check/environment.rs, crates/xtask/src/mutants_check/scope.rs, crates/xtask/src/mutants_check/scope_tests.rs, crates/xtask/src/source_check/tests.rs, crates/xtask/src/test_runner_tests.rs, justfile, harness/check-list/justfile.json, harness/source-checks/allowlists.toml, harness/claude-hooks/hashes.json, .codex/config.toml, docs/adr/h-42-mutation-scope.md. lock-fragment.json도 쓰기 모드로 재생성했으나 차이가 없다. 변경은 인덱스에 스테이징했다.
