# ADR 0083: D-42 변이 미검출 보강

- 상태: 확정 (2026-11-16)


## 결정

기준은 `8ba1f6abf3b1050017b649d1d1a5fd63104a6363`이다. 현행 D-29·D-37 대상 8파일 전체의 515개 변이를 먼저 실행한 뒤 테스트만 추가한다. 기존 테스트 본문과 제품 동작은 변경하지 않는다. 과거 H-46의 건수 대신 이 기준의 실제 목록을 사용한다. H-46은 D-29 미검출 15건·시간 초과 3건, D-37 미검출 4건을 기록했으나 현행 전 측정은 D-29 미검출 10건·시간 초과 3건, D-37 미검출 4건이다. 이번에 확인한 작업 사본과 관련 Git 기록에서 H-46 당시 변이별 목록·대상 SHA·실행 설정을 찾지 못해 차이 5건을 일대일 대응하지 못했다. 차이의 원인은 미확인이다. 과거 5건을 임의로 특정하거나 무작위성 때문이라고 단정하지 않는다.

설치된 cargo-mutants 27.1.0의 `--help`와 justfile 및 xtask 레시피를 확인했다. `--no-config -p domain -f <파일명>`으로 파일을 한정한다. 설정의 examine_globs와 파일 옵션을 함께 사용하면 전체 대상이 선택되므로 `--no-config`와 필수 `--cap-lints=true --test-tool=nextest`를 명시했다. 파일별 출력 경로를 분리하고 실행당 1,195초 뒤 소유 프로세스 그룹만 종료하도록 했다. 모든 기본 실행이 제한 안에 완주했다.

전 측정은 원본 domain 전체 900개 테스트로 실행했다. 후 측정은 추가 테스트를 포함한 911개, 마지막 bootstrap 재측정은 912개 테스트로 실행했다. `STOCK_COPILOT_PROPTEST_CASES`를 제거하여 기본 256회를 사용했다. 프로필·병렬도는 xtask와 같은 debug=0, opt-level=1, debug assertions 및 overflow checks 활성화, 빌드 작업 수 1, 변이 작업 6개, 테스트 스레드 2개다. 로그와 JSON은 작업 사본의 `target/D-42/`에 있다.

## 전후 측정

아래 표는 테스트 스레드 2개로 각 파일을 실행한 실측 기록을 취합한 것이다. 스레드 수나 실행 순서가 달라도 같은 결과가 보장된다는 뜻이 아니다.

순서는 검출 / 미검출 / 시간 초과 / 컴파일 불가다. 컴파일 불가를 검출로 세지 않는다.

| 파일 | 전 | 후 |
|---|---|---|
| bootstrap.rs | 39 / 1 / 3 / 16 | 43 / 0 / 0 / 16 |
| signal_return.rs | 34 / 1 / 0 / 32 | 35 / 0 / 0 / 32 |
| signal_benchmark.rs | 16 / 2 / 0 / 7 | 18 / 0 / 0 / 7 |
| virtual_exit.rs | 87 / 6 / 0 / 42 | 93 / 0 / 0 / 42 |
| market_ranking.rs | 47 / 1 / 0 / 25 | 48 / 0 / 0 / 25 |
| investor_flow.rs | 39 / 2 / 0 / 28 | 39 / 2 / 0 / 28 |
| market_index.rs | 32 / 0 / 0 / 32 | 32 / 0 / 0 / 32 |
| sector_composition.rs | 9 / 1 / 0 / 13 | 10 / 0 / 0 / 13 |
| 합계 | 303 / 14 / 3 / 195 | 318 / 2 / 0 / 195 |

동치 2개를 제외한 미검출은 0이다. 시간 초과 0은 이 2스레드 실행의 관측값이다. 독립 비판자의 같은 변이 프로필·4스레드 재현에서는 아래 세 변이가 모두 시간 초과했다. 시간 초과의 안정적 해소라는 결론은 철회하며, 허용 설정에 남는 실패 3개를 별도 기록한다.

## 동치

위치는 기준 SHA의 줄이다. 허용 목록이나 변이 설정을 변경하지 않는다.

| 파일:줄 | 변이 | 관측 동치의 이유 |
|---|---|---|
| investor_flow.rs:462 | validate_institution 첫 `||`를 `&&`로 치환 | 원식은 `x || y || z`, 변이식은 `(x && y) || z`다. x는 기관 매수 합계 차이, y는 매도 차이, z는 순매수 차이다. 모든 행의 순매수는 정수 매수−매도로 검증되며, 합계의 checked 산술 오류는 비교 전에 반환된다. z가 거짓이면 두 차액이 같으므로 x와 y의 진릿값도 같다. z가 참이면 두 식 모두 참이다. 따라서 모든 도달 가능한 입력에서 동일 오류 또는 동일 사실을 반환한다. 중복 조건이며 제품 코드는 유지한다. |
| investor_flow.rs:463 | validate_institution 둘째 `||`를 `&&`로 치환 | Rust 연산자 우선순위에 따라 변이식은 `x || (y && z)`다. x가 참이면 두 식 모두 참이다. x가 거짓이면 매수 합계가 같으므로 매도 합계 차이와 순매수 차이는 동시에 존재하거나 동시에 없다. 따라서 y와 z의 진릿값이 같아 두 식이 동일하다. 첫 변이와 마찬가지로 산술 오류는 조건 전에 처리된다. |

## 시간 초과

| 파일:줄 | 변이 | 원인과 처리 |
|---|---|---|
| bootstrap.rs:180 | Generator::index `>` → `==` | 기존 65,537개 묶음 입력이 거부되지 않는다. ceiling=0이 되어 어떤 후보도 수용하지 못한다. |
| bootstrap.rs:190 | 후보 `<` → `==` | 묶음 수가 2의 거듭제곱이면 ceiling=65,536으로 u16 후보의 범위 밖이다. 종료 불가다. |
| bootstrap.rs:190 | 후보 `<` → `>` | 같은 입력에서 ceiling보다 큰 u16 후보가 없어 종료 불가다. |

셋 모두 기본 실행에서 테스트 제한 144초로 시간 초과했다. 같은 변이를 유한한 잘못된 반환값으로 관측하는 경계 입력을 추가했다. 2스레드의 59개 재측정은 43개 검출·16개 컴파일 불가, 종료 0이었다. 그러나 새 테스트의 유한한 실패만으로 실행 전체의 종료가 보장되지는 않는다. 기존 d29_cluster_accumulation_and_sample_integer_overflow_are_errors와 splitmix_words_rejection_and_seed_accessor_have_fixed_integer_witnesses는 같은 변이에서 여전히 종료 불가능한 입력을 호출한다. nextest fail-fast는 이미 실행 중인 테스트가 끝나기를 기다리므로 이 테스트가 먼저 시작되면 전체 실행은 시간 초과한다.

묶음 수 65,533의 원래 첫 후보는 시드 0에서 52,655, 시드 42에서 28,309여서 거절 없이 선택된다. 독립 Python 정수 계산에서 시드 0의 후보 65,533은 20,362번째,65,534는 12,508번째에 나온다. 따라서 `< → ==`는 0, `< → >`는 1을 반환해 첫 후보 기대값과 다르고 유한하게 실패한다. 허용 상한 65,536은 `> → ==`가 잘못 거부하므로 즉시 실패한다. 제품 알고리즘·기존 테스트·시간 제한을 바꾸지 않았다. 순수한 정수 예제이며 스레드·시계나 새 의존성이 없다.

### 허용 스레드 설정의 독립 재현

새 Codex 비판자는 대상 6fbdf7cec19f55bcee02a4e2beb41093e104a1f6의 복사본에서 cargo-mutants 27.1.0·nextest 0.9.151, opt-level 1·debug 0·debug assertions/overflow checks 활성화, 속성 256회, Cargo 빌드 작업 1개, 무필터 domain912개 전체 테스트로 원래 세 변이만 실행했다. 각 설정의 무변이 기준은 912 passed·0 skipped였다. 모든 실행이 20분 안에 끝났다.

| 변이 | 2스레드 | 3스레드 | 4스레드 |
|---|---|---|---|
| bootstrap.rs:180 `>`→`==` | 검출 2.967초 | 검출 1.455초 | 시간 초과 58.046초 |
| bootstrap.rs:190 `<`→`==` | 검출 2.241초 | 검출 2.065초 | 시간 초과 58.019초 |
| bootstrap.rs:190 `<`→`>` | 검출 2.082초 | 검출 1.856초 | 시간 초과 58.017초 |
| 실행 종료 코드 | 0 | 0 | 3 |

시간은 JSON의 Test 단계다. 전체 실행 시간은 각각 115.94초·58.60초·218.62초다. 4스레드에서는 새 경계 테스트가 실패하기 전에 기존 overflow 테스트가 시작됐고, 실패 뒤에도 남아 있다가 제한 시간에 SIGTERM으로 종료됐다. 실행 명세·결과·로그의 읽기 전용 사본은 작업 사본의 target/D-42/critic-evidence/mutants-t2·mutants-t3·mutants-t4에 보존했다.

새 테스트만 추가하는 범위에서는 이미 실행된 다른 테스트 프로세스의 무한 반복을 확실하게 종료할 수 없다. 기존 테스트 호출을 종료 가능한 구조로 바꾸거나 nextest의 실행 중 테스트 종료 정책을 바꾸려면 허용 범위를 벗어난 변경이 필요하다. 작업 지시의 “불가능하면 기록” 경로를 적용한다. 경계 예제는 유지하고, 스레드 수·실행 순서를 고정해서 문제를 숨기지 않으며, 시간 초과를 계속 실패로 취급한다. 허용 목록이나 검사 설정은 바꾸지 않는다.


## 인계

- D-05: 기존 portfolio_valuation.rs의 assert_independent_rate ZeroCost 분기는 무작위 0 수량 표본에 의존한다. 전체 검사에서 한 번 미측정됐고 후속 4,096회 실행에서는 측정됐다. 고정 0 수량 예제로 계측을 결정적으로 만드는 보강은 D-05 범위로 넘긴다. 기존 테스트는 수정하지 않았다.
- H-47: 하네스의 4초 PID 테스트는 패키지 캐시 잠금 대기 중 child.pid가 생성되기 전에 종료될 수 있었다. 변이 실행 종료 뒤 단독 재현과 후속 정상 전체 훅에서는 통과했다. 날짜별 전체 변이의 완주·시간 초과 정책을 보강할 때 이 경합을 추적한다. 새 경계 테스트는 유한한 실패를 제공하지만, 허용 4스레드에서는 기존 테스트의 무한 반복 때문에 세 시간 초과가 남는다. 기존 테스트 호출의 종료 보장 또는 실행 중 테스트 종료 정책은 H-47에서 결정한다. 시간 초과 실패 정책은 유지한다. H-46 당시 변이별 목록·대상 SHA·설정 복구도 H-47 인계이며, 확보 전에는 과거 5건과 현행 결과를 대응했다고 주장하지 않는다.
- D-42 병합 담당: 수급의 중복 조건을 향후 정리할 때 위 정수 관계를 유지한다. 기존 제품 코드·기존 테스트 본문 변경은 없다.
- 프로젝트 설정 `hooks=true`와 lefthook 설치를 확인했다. 사용자별 프로젝트·훅 신뢰 실제 적용 상태는 홈 설정을 읽지 않아 검증하지 못했으며 실제 검사 실행과 Git 훅 결과를 별도 근거로 사용한다.

## 검사와 리뷰

- 새 예제 테스트는 최초 11개에 난수 경계 1개를 추가해 최종 12개다. 집중 실행:12 passed. domain 전체 대상 clippy 통과. 검사 끄기·외부 의존성 추가 없음. 기존 테스트 본문은 변경하지 않았다.
- 손 변형 결과에 따라 만기일 장중 분봉 관측을 보강했다. 최종 자동 전수의 virtual_exit는 이 단언을 포함한다. 이후 도우미의 전체 경로를 use로 줄였으며 단언과 입력은 같다.
- 자동 전후 각각 515개의 변이 이름 집합이 파일별로 같은지 JSON으로 대조했다. 최종 미검출 2개는 위 동치 목록과 정확히 같다. 동치를 제외한 미검출 0이다. 2스레드 전수의 시간 초과는 0이지만, 4스레드의 세 변이 재현에서는 시간 초과 3개가 남는다. bootstrap은 경계 예제를 추가한 뒤 같은 59개를 한 번 더 실행했고 이름 집합도 동일했다. 이 2스레드 실행의 테스트 시간 제한은 127초였으며 세 변이 모두 새 테스트의 실패로 검출됐다. 이 관측값을 모든 허용 스레드 설정의 종료 보장으로 해석하지 않는다.
- signal_benchmark의 첫 재실행은 커밋 훅의 의존성 재설치와 파일 복사가 겹쳐 I/O 오류로 종료 1이었다. 이를 전수 결과로 세지 않고, 재설치가 끝난 뒤 25개 전체를 다시 실행해 종료 0을 얻었다. 다른 모든 파일도 실행당 20분 안에 완주했다.
- `just mutants-diff 8ba1f6abf3b1050017b649d1d1a5fd63104a6363`: 종료 0, 선택 변이 0개. application·domain·order-execution을 밤 전체 대상으로 알렸다. 테스트 추가만 있는 변경분의 선택 결과이므로 검출력 증거로 세지 않는다.
- 정상 커밋 훅 첫 실행은 새 테스트 63줄이 60줄 제한을 넘어서 실패했다. 도우미 경로 정리로 해결하고 clippy를 통과했다. 둘째 실행은 전체 1,304개 중 1,219개 통과·1개 실패·84개 미실행이었다. 기존 하네스의 deadline_stops_nextest_child_groups_and_preserves_unrelated_pid가 child.pid를 읽지 못했고 패키지 캐시 잠금 대기가 있었다. 하네스·기존 테스트를 바꾸지 않고 변이 실행 종료 뒤 단독 재현과 정상 훅으로 확인한다.
- 새 ADR 등록을 위해 `.codex/config.toml`과 `harness/claude-hooks/hashes.json`을 저장소 스크립트로 재생성했다. `harness/sandbox/fragment.mjs --write`도 실행했으며 결과 변경은 없었다. 보호 규칙은 완화하지 않았다.
- 변이 실행 종료 뒤 기존 하네스 PID 테스트를 단독 실행했다: 1 passed, 296 skipped, 종료 0. 하네스와 기존 테스트는 수정하지 않았다.
- 셋째 정상 훅은 전체 1,304개 테스트를 통과했다. 커버리지 테스트 956개도 통과했지만 새 테스트의 실패 전용 panic 줄 3곳과 기존 portfolio_valuation.rs:1576이 미실행으로 보고돼 커버리지 판정이 실패했다. 새 테스트는 결과 종류·내용을 직접 단언하는 형태로 정리했다. 기존 포트폴리오 속성 테스트는 0 수량을 생성할 수 있으나 고정 예제가 없어 해당 분기의 측정 여부가 표본에 의존한다. 이 테스트를 수정하지 않았다.
- 단언 정리 후 `cargo clippy --locked -p domain --all-targets -- -D warnings` 통과. `STOCK_COPILOT_PROPTEST_CASES=4096 just coverage-check`:956 passed, `coverage-check: 위반 없음`, 종료 0. 커버리지 예외나 정책 변경은 없다.
- 최종 단언 형태에서도 손 변형 22개를 모두 재실행했다: 21개 검출, 1개 동치 생존, 컴파일 실패 0, 시간 초과 0. 원본 집중 테스트 11개 통과 뒤 실행했으며 임시 복사본 복원을 바이트 비교로 확인했다.
- 구현 커밋 d8e7a177b74ebeca55507da15cd0caa21335e1c5의 정상 훅 통과:전체 1,304 passed, 커버리지 956 passed·위반 없음, test-diff·hooks-installed 위반 없음. 검사 끄기 없이 종료 0이다.
- 이후 난수 경계 테스트 1개를 추가했고, clippy 및 집중 12개 테스트가 통과했다. bootstrap 59개의 2스레드 재측정에서 시간 초과 3개도 검출됐으나, 후속 4스레드 재현에서는 세 시간 초과가 남았다. 이 추가 테스트는 아래 22개 손 변형 뒤 추가했으며 기존 테스트를 변경하지 않는다.
- 난수 보강의 첫 후속 훅은 기존 PID 경합으로 실패했다. 이후 최신 master의 31e73d4d8056b6d088e2c819ac589987e7d1b19a(기한 4→60초·대기 fixture900초, 단언 유지)를 받았다. 이 기존 테스트 변경은 master에서 병합한 것이며 D-42가 직접 고치지 않았다.
- master 0739990f8eaa8db529dd13b3e6e7b6d01809140b를 `git merge --no-ff --no-edit master`로 받았다. 충돌은 생성된 Codex 설정·훅 해시뿐이었고 저장소 스크립트로 재생성했다. 병합 커밋 5f99c9dd24a46314bbdf3eb117b163cb5794992e의 정상 전체 훅:1,324 passed, 커버리지 976 passed·위반 없음, test-diff 위반 없음, 종료 0.
- 미커밋 난수 보강과 ADR 두 파일을 별도 Git 객체·패치로 보존했다가 병합 후 복원했고 보존 객체와 바이트 일치를 확인했다. 최초 변이 기준 8ba1f6abf3b1050017b649d1d1a5fd63104a6363 대비 8대상 파일의 첫 테스트 모듈 앞 제품 소스가 모두 바이트 단위로 같은지 확인했다. 해시는 작업 사본의 product-prefix-check.json에 있다.
- 독립 리뷰의 기준은 병합한 master 0739990f8eaa8db529dd13b3e6e7b6d01809140b로 두어 병렬 A-02·DS-05·DS-21 변경을 D-42 diff에 섞지 않는다. 변이 전 측정 기준은 최초 SHA를 유지한다.
- 최종 구현 커밋 6fbdf7cec19f55bcee02a4e2beb41093e104a1f6의 정상 전체 훅:1,325 passed, 커버리지 977 passed·위반 없음, test-diff 위반 없음, 종료 0. 중간 실행은 기존 Git fixture 하위 프로세스 7개가 시작 지점에서 고착돼 소유 PID를 확인해 중단·정리했고 통과로 세지 않았다. 같은 실행 파일의 독립 호출은 즉시 정상 반환했으며, 정상 훅 전체 재실행으로 위 통과 결과를 얻었다.
- 사용자 지정에 따라 새 Opus 5.5 리뷰 1회와 새 Codex gpt-6.1-sol 비판 1회를 수행했다. Opus는 중요 1·사소 2건을 냈고, 비판자는 각각 중요 확정·기각·불확실로 판단했다. 실제 변이 프로필의 4스레드 시간 초과가 독립 재현돼 측정의 범위와 종료 보장 한계를 정정했다. 생성자·접근자 단언은 계산 결과의 PartialEq와 별도 계약을 고정하므로 유지했다. H-46 과거 5건의 대응 한계도 명시했다. 후속 변경은 문서와 리뷰 기록이며 제품·테스트·하네스는 변경하지 않는다. 상세 판정은 docs/reviews/D-42.md에 있다.
- 리뷰 후 master 580a19874ee4ad32c21c4c3f83a0a4142b3a4858를 병합 커밋 604b077f7c407eb1d9cf520a3c99916df17a8cde로 받았다. 정상 전체 훅은 1,335 passed, 커버리지 987 passed·위반 없음, test-diff·hooks-installed 위반 없음, 종료 0이다. 로그는 target/D-42/merge-master-final-commit.log다.

## 손 변형

임시 복사본에서 실행 후 변형별 결과를 기록한다. 컴파일 실패와 실행하지 않은 변형은 성공으로 세지 않는다.

손 변형 최종 실행은 원본 집중 테스트 11개가 통과한 임시 복사본에서 21개를 재실행하고, 기관의 순매수만 비교하는 변형 1개를 추가 실행했다. 합계 22개 중 21개는 새 테스트 실패로 검출됐고, 1개는 위에 증명한 수급 동치 변이로 생존했다. 컴파일 실패 0, 시간 초과 0이다. 초기 실행에서 매도 분봉 변이 2개가 생존해 만기일 장중 단언을 추가한 뒤 21개 전부 재실행했다. 최종 실행 후 복사본의 변형 대상 소스가 원본과 같은지 바이트 비교했다. 전수 변이 후에는 같은 입력·기대값을 유지하며 도우미 경로를 use로 줄이고, 커버리지에 잡힌 실패 전용 분기를 직접 단언으로 정리했다. 최종 테스트 형태에서도 22개 전부 재실행해 같은 검출·동치 결과를 확인했다.

| 번호 | 변형 | 결과 | 검출 테스트 |
|---|---|---|---|
| 1 | crates/domain/src/bootstrap.rs:41:9: replace WithheldCount::as_usize -> usize with 1 | 컴파일 성공·검출 | signal_return_review::tests::d42_multiple_missing_comparisons_preserve_the_withheld_count |
| 2 | crates/domain/src/investor_flow.rs:463:9: replace &#124;&#124; with && in validate_institution | 컴파일 성공·동치 생존 | 동치 표 investor_flow.rs:463 |
| 3 | crates/domain/src/market_ranking.rs:305:36: replace < with <= in ranking_item | 컴파일 성공·검출 | market_ranking::review_tests::d42_zero_amount_and_volume_are_available_ranking_facts |
| 4 | crates/domain/src/sector_composition.rs:134:44: replace < with <= in sector_amount | 컴파일 성공·검출 | sector_composition::tests::d42_zero_sector_money_remains_available_and_distinct_from_absence |
| 5 | crates/domain/src/signal_benchmark.rs:26:28: replace > with >= in SignalBenchmark::from_candles | 컴파일 성공·검출 | signal_benchmark::tests::d42_calendar_limit_accepts_4097_consistent_facts |
| 6 | crates/domain/src/signal_benchmark.rs:51:39: replace > with >= in SignalBenchmark::from_candles | 컴파일 성공·검출 | signal_benchmark::tests::d42_distinct_settlement_days_can_share_the_previous_market_close |
| 7 | crates/domain/src/signal_return.rs:449:9: replace &#124;&#124; with && in validate_rates | 컴파일 성공·검출 | signal_return_review::tests::d42_each_combined_cost_side_has_an_independent_inclusive_limit |
| 8 | crates/domain/src/virtual_exit.rs:365:56: replace && with &#124;&#124; in evaluate_rows | 컴파일 성공·검출 | virtual_exit::d42_tests::d42_daily_selection_uses_the_current_trading_day<br>virtual_exit::d42_tests::d42_sell_minutes_and_daily_rows_wait_for_the_recorded_maturity |
| 9 | crates/domain/src/virtual_exit.rs:367:74: replace < with > in evaluate_rows | 컴파일 성공·검출 | virtual_exit::maturity_tests::d42_elapsed_intraday_maturity_still_uses_the_minute_close |
| 10 | crates/domain/src/virtual_exit.rs:428:91: replace == with != in entry_close | 컴파일 성공·검출 | virtual_exit::d42_tests::d42_regular_session_intraday_entry_is_not_a_close |
| 11 | crates/domain/src/virtual_exit.rs:450:34: replace == with != in daily_exit | 컴파일 성공·검출 | virtual_exit::d42_tests::d42_sell_minutes_and_daily_rows_wait_for_the_recorded_maturity |
| 12 | crates/domain/src/virtual_exit.rs:551: 21: replace == with != in scan_minutes | 컴파일 성공·검출 | virtual_exit::d42_tests::d42_sell_minutes_and_daily_rows_wait_for_the_recorded_maturity |
| 13 | crates/domain/src/virtual_exit.rs:551:28: replace && with &#124;&#124; in scan_minutes | 컴파일 성공·검출 | virtual_exit::d42_tests::d42_sell_minutes_and_daily_rows_wait_for_the_recorded_maturity |
| 14 | 보류 건수 상수 0 | 컴파일 성공·검출 | signal_return_review::tests::d42_multiple_missing_comparisons_preserve_the_withheld_count |
| 15 | 비교 불가 건수 두 배 증가 | 컴파일 성공·검출 | signal_return_review::tests::d42_multiple_missing_comparisons_preserve_the_withheld_count |
| 16 | 비교 불가/근사 건수 바꿈 | 컴파일 성공·검출 | signal_return_review::tests::d42_multiple_missing_comparisons_preserve_the_withheld_count |
| 17 | 청산 가격을 진입 가격으로 교체 | 컴파일 성공·검출 | virtual_exit::d42_tests::d42_daily_selection_uses_the_current_trading_day<br>virtual_exit::d42_tests::d42_sell_minutes_and_daily_rows_wait_for_the_recorded_maturity<br>virtual_exit::maturity_tests::d42_elapsed_intraday_maturity_still_uses_the_minute_close |
| 18 | 시가총액을 거래대금으로 교체 | 컴파일 성공·검출 | sector_composition::tests::d42_zero_sector_money_remains_available_and_distinct_from_absence |
| 19 | 거래대금을 시가총액으로 교체 | 컴파일 성공·검출 | sector_composition::tests::d42_zero_sector_money_remains_available_and_distinct_from_absence |
| 20 | 거래량을 거래대금 입력으로 교체 | 컴파일 성공·검출 | market_ranking::review_tests::d42_zero_amount_and_volume_are_available_ranking_facts |
| 21 | 거래대금을 거래량 입력으로 교체 | 컴파일 성공·검출 | market_ranking::review_tests::d42_zero_amount_and_volume_are_available_ranking_facts |
| 22 | 기관 검증을 순매수 합계만 비교하도록 축소 | 컴파일 성공·검출 | investor_flow::tests::d42_equal_net_does_not_excuse_wrong_buy_and_sell_totals |
