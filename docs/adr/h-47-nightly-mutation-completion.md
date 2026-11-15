# 밤 전체 변이 실행의 완주

- 상태: 초안

## 기준과 진단

기준 커밋은 `8ba1f6abf3b1050017b649d1d1a5fd63104a6363`이다. 원격 조회는 `gh run list --workflow nightly.yml`, 두 실행의 `gh run view --log`만 사용했다. 원격 쓰기는 하지 않았다.

| 실행 | 대상 | 진단 |
|---|---|---|
| 43644660038, 2026-11-14 | 변이 네 조각 | 120분에 모두 취소. 대상은884/884/884/881개, jobs2, 기준 실행은빌드20~22초+테스트84~86초였다. 120분에서 준비를 제외한 처리 예산은 변이당 약16초인데 로그에 남은 미검출은 빌드4~10초+테스트114~162초였다. 미검출 로그는 느린 경로의 표본이며 전체 변이의 평균으로 단정하지 않는다. 해당 과거 실행의 timeout multiplier는10, 현행은5다. 이전 ADR의 작은 표본을 현행 전체 대상에 적용한 추정이 부족했다. |
| 43523619697, 2026-11-13 | macOS check-ts | ds-final-runtime.test.mjs와 ds-runtime.test.mjs에서 functional/prefer-immutable-types 실행 중 TypeScript normalizeSlashes의 undefined.includes 예외. |
| 43644660038, 2026-11-14 | macOS 전체 검사 | 22:23:13 UTC hooks-installed까지 통과. a291516a7808c7fe2468cf0e0014c7399ce44583의 임시 경로 및 타입 프로그램 격리 수정이 이미 포함됨. |

macOS 실패는 현행 master에서 수정된 문제이므로 다시 코드를 바꾸지 않는다. 기존 두 실행 테스트와 ds-runtime-environment.test.mjs를 현행에서 재실행했다: 4/4 통과. 타입 프로그램의 single-run 캐시와 반복해서 교체하는 임시 소스를 격리하는 기존 회귀를 유지한다.

## 결정

밤 분할은 0/128부터 127/128까지 정확히 128개다. cargo-mutants 27.1.0의 기본 slice 분할과 no-shuffle을 유지한다. 현행 전체 목록 4,452개에서 조각당 최대 ceil(4452/128)=35개다. CLI의 실제 각 조각 목록을 모아 비중복과 전체 합집합을 검증하는 회귀를 추가한다. xtask는 정확한 표기만 받고 예전 네 조각과 비정규 표기는 거부한다. justfile은 분할 값을 전달하기만 하므로 변경할 필요가 없다.

CI의 기존 로그에서 느린 생존 변이 비용을 172초(빌드10+테스트162)로 잡으면, 4 vCPU 러너의 jobs=2에서 35×172/2=3,010초, 약50.2분이다. 설치·baseline20분과 변이 비용 두 배 여유를 더하면 약120.4분이다. 조각 단계 제한160분, 작업 제한180분으로 설치와 always 요약에 여유를 둔다. 기존 정책의 최대180분은 늘리지 않는다. 표본 및 러너 편차를 반영한 추정이며 원격 완주 증거가 아니다. 대량 timeout은 여전히 단계 제한으로 실패할 수 있다.

변이 단계에만 timeout-minutes=160을 허용한다. 다른 단계와 값은 정책에서 거부한다. fail-fast=false와 미검출·시간 초과 실패 판정은 유지한다. always 요약은 조각과 실행 outcome, 예정 수와 완료 수에 따른 완주/불완전을 로그와 작업 요약에 남기며, 예정 목록·완료 자료가 없거나 baseline 실패·취소·부분 실행이면 성공으로 표시하지 않는다. 작업 자체가 강제 종료되거나 러너가 유실되면 always 실행을 보장할 수 없으므로 별도 단계 제한으로 작업 제한 도달을 피한다.

## 승인된 변경

보호 경로 `.github/workflows/nightly.yml`, `harness/ci/mutants-summary.mjs`, `harness/ci/workflow-policy/check.mjs`, `harness/ci/workflow-policy/policy.json`, 관련 CI 테스트, `crates/xtask/src/mutants_check.rs` 및 tests.rs를 바꾼다. 새 ADR 등록에 따른 생성물은 저장소 생성기로 갱신한다. 새 외부 의존성과 검사 끄기는 없다.

기존 테스트의 기계적 기대값 갱신: `h42-nightly.test.mjs`의 조각 수4→128, 작업 제한120→180. `mutants_check/tests.rs`의 shard_selection_accepts_only_fixed_partition에서 유효 목록0/4..3/4→0/128..127/128, 상한 거부4/4→128/128. workflow-policy.test.mjs의 정확한 stepKeys 기대 목록에는 timeout-minutes 한 항목을 추가한다. 나머지 단언은 유지한다.

## 인계

원격 미검증, 다음 밤 실행에서 확인. 메인은 병합·푸시 후 128개 조각 각각의 준비·baseline·변이 소요 시간과 완료 수, 생존·timeout 실패 여부, 실패·취소 시 로그 및 작업 요약 유무를 확인한다. 의도적으로 취소한 실행에서도 요약이 남는지 확인한다. 실제 계좌 및 키는 사용하지 않는다.

D-42와 이후 작업은 현행 미검출의 테스트 보강을 받는다. 밤 실행이 현행 미검출 때문에 실패하는 것은 의도된 동작이다. 현행 미검출과 실측 결과는 아래 실행 증거에 기록한다.

## 실행 증거

현행 master 소스에서 같은 자식 프로필·256회·jobs2·nextest 스레드2로 작은 slice 표본 200/1024를 실행했다. 5개 중 컴파일되는 변이2개는 검출, 컴파일 불가3개, 미검출0, timeout0이다. baseline은 빌드73.077초+테스트41.868초. 컴파일되는 두 변이의 빌드+테스트는26.103초와125.051초, 평균75.577초다. 전체 표본은290.341초이며 준비와 컴파일 불가를 포함한 변이당58.068초다. 동시 작업에 의한 빌드 편차가 크고 표본이 작으므로 CI 완료 시간은 이 평균을 보장값으로 쓰지 않고 앞의 원격172초 및 두 배 여유로 추정한다. 본 실측에서 현행 미검출은 발견되지 않았다. 첫 표본0/1024는5개 전부 컴파일 불가여서 유효 변이 성능 근거로 쓰지 않았다.

새 요약 회귀3개 및 기존 요약 회귀, 정책 회귀8개, macOS 회귀4개, xtask 변이 관련15개가 통과했다. 손 변형21개는 임시 복사본에서 실제 실행해 모두 검출했다. 변경분 변이와 첫 check-fast는 실행 중 signal9로 중단됐다. 성공으로 세지 않고 정상 커밋 훅과 후속 검사의 결과로 갱신한다.

프로젝트 설정과 hooks.json은 존재하나 T3 런타임의 별도 훅 신뢰 수락을 이 세션에서 증명하지 못했다. Git lefthook 설치와 실제 정상 커밋 검사를 별도로 확인한다.

## 손 변형

작업 사본 target/H-47 아래 mktemp 임시 복사본에서 JS를 실제 실행했다. 각 변형은 node --test의 H47 cancellation/completion/timeout 테스트로 검증했다. 파싱 오류는 없었고 21개 모두 단언 실패로 검출됐다. 첫 실행의 생존 3개는 새 테스트를 보강한 뒤 재실행했다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | `outcomes.total_mutants === planned.length` 변경 | H47 completion | 검출 |
| 2 | `outcomes.total_mutants === planned.length` 변경 | H47 completion | 검출 |
| 3 | `outcomes.total_mutants === planned.length` 변경 | H47 completion | 검출 |
| 4 | `planned.length > 0` 변경 | H47 completion | 검출 |
| 5 | `entry.scenario === "Baseline"` 변경 | H47 completion | 검출 |
| 6 | `entry.summary === "Success"` 변경 | H47 completion | 검출 |
| 7 | `&& entry.summary` 변경 | H47 completion | 검출 |
| 8 | `state = "완주"` 변경 | H47 completion | 검출 |
| 9 | `let state = "불완전"` 변경 | H47 cancellation | 검출 |
| 10 | `state = "불완전"; }` 변경 | H47 cancellation | 검출 |
| 11 | `outcome !== "success"` 변경 | H47 completion | 검출 |
| 12 | `state !== "완주"` 변경 | H47 completion | 검출 |
| 13 | `|| state !== "완주"` 변경 | H47 completion | 검출 |
| 14 | `process.exitCode = 1` 변경 | H47 cancellation | 검출 |
| 15 | `"missed.txt"` 변경 | H47 cancellation | 검출 |
| 16 | `"timeout.txt"` 변경 | H47 cancellation | 검출 |
| 17 | `process.env.MUTATION_SHARD` 변경 | H47 cancellation | 검출 |
| 18 | `appendFileSync(process.env.GITHUB_STEP_SUMMARY, summary)` 변경 | H47 cancellation | 검출 |
| 19 | `step["timeout-minutes"] === 160` 변경 | H47 timeout | 검출 |
| 20 | `step.name === "mutants without network"` 변경 | H47 timeout | 검출 |
| 21 | `job === "mutants"` 변경 | H47 timeout | 검출 |


## 추가 승인: 부하에 민감한 종료 회귀

메인의 추가 지시로 `mutants_check::environment::tests::deadline_stops_nextest_child_groups_and_preserves_unrelated_pid`의 기존 본문 변경을 승인받았다. 단언은 유지하고 준비·대기 방식을 바꾼다. 기존 코드는 fixture 사전 빌드 후 다시 cargo nextest run을 실행하며 4초 안에 테스트 자식이 시작할 것으로 가정했다. Cargo 캐시 잠금과 재빌드가 지연되면 child.pid가 없고, 자식이 시작되기 전에 부모를 종료해 FAIL 또는 LEAK가 될 수 있었다. 무관한 sleep60도 부하에 따라 자연 종료할 수 있었다.

빌드와 Cargo·바이너리 메타데이터 준비는 기한 측정 전에 끝낸다. 실행은 cargo-nextest에 두 메타데이터 파일을 직접 넘기며 CARGO=/bin/false로 재빌드나 Cargo 호출이 생기면 실패하게 한다. PID 파일이 실제 테스트 자식의 PID를 담을 때까지 최대180초 조건 대기 후 기한 검사를 시작한다. 자식은 고정60초 sleep 대신 release 파일이 생길 때까지 조건 대기한다. 무관한 프로세스도 파이프가 열린 cat으로 유지한다. 기한 함수가 반환한 직후의 자식 상태를 즉시 판정한다. 최대180초 조건 대기는 실패한 fixture 정리에만 사용하며 판정에 유예를 주지 않는다. 기존의 기한 실패·자식 종료·무관한 PID 생존 단언은 그대로다. 정상 종료 명령의 1초 상한도30초로 바꾼다. 준비 실패나 단언 실패에도 release 파일로 fixture 자식을 해제하고 소유 Child를 정리·reap하는 테스트 전용 가드를 둔다. 이미 종료된 숫자 PID·PGID에 신호를 보내지 않는다.

운영 기한 계산은 기존 시작 시점을 유지한 채 wait_with_limit으로 분리했다. 테스트만 자식 준비 뒤 같은 종료 루프를 사용한다. 운영20분·SIGINT·SIGKILL 실패 정책은 유지한다. 추가 보호 변경은 crates/xtask/src/mutants_check/environment.rs다.

추가 조사 목록: git_hooks.rs와 git_hooks/**에는 sleep·짧은 deadline·비동기 spawn/try_wait 기반 테스트를 찾지 못했다. 동기 Git 자식의 종료 결과를 기다리는 방식이다. network_sandbox_tests.rs의 CONNECT_TIMEOUT5초와 nc -G5는 외부 연결이 즉시 PermissionDenied로 거부되는지를 보는 상한으로, 준비 완료를 시간으로 추정하는 테스트와 다르므로 바꾸지 않았다. private_temp.rs의1시간은 파일 수정 시각의 stale 정책이며 실제 대기를 요구하지 않는다.

새 대기 방식의 nextest 단독 회귀는22.465초에 PASS, LEAK 없음. CARGO=/bin/false에서도 미리 빌드한 테스트가 시작되고 종료 단언을 통과했다. 지정 PATH를 적용한 관련 xtask·git_hooks 묶음은74/74 통과(50.760초), 종료 회귀 자체는25.301초, LEAK 없음. 실행 전 같은 기계의 load averages는67.06/72.47/86.78이었다. 정확히 load100을 재현했다고 주장하지 않는다. 첫 묶음 실행에서는 지정 pnpm PATH가 빠져 corepack pnpm 모듈을 못 찾았으며, 올바른 PATH로 다시 실행해 모두 통과했다. 이는 시간 대기 실패가 아니므로 당시에는 git_hooks 테스트를 수정하지 않았으며, 후속 동시 실행 조사는 아래에 기록한다. xtask 전체 대상 Clippy도 통과했다. 정상 훅 결과는 후속 기록으로 갱신한다.


## 커밋 검사와 승인 예외

정상 커밋 훅의 전체 검사에서1294/1294 workspace 테스트와945/945 커버리지 테스트가 통과했고 coverage-check 위반은 없다. 최종 test-diff는 승인된 두 Rust 기존 테스트의 본문 변경만 지적한다: shard_selection_accepts_only_fixed_partition의 정확한 분할 목록 갱신과 추가 승인된 deadline_stops_nextest_child_groups_and_preserves_unrelated_pid의 대기 방식 변경. 새 import의 shadowing 두 건은 완전한 경로 표기로 제거하고 test-diff를 다시 실행해 위 두 건만 남는 것을 확인했다. 이 표기 변경 뒤 xtask 전체 대상 Clippy도 통과했다.

나머지 전체 검사 단계는 통과했고, test-diff 뒤 단계인 disable-report·disable-report-since·hooks-installed도 따로 통과했다. 새 끄기는0건이다. 사용자가 지시한 승인 예외에 따라 승인된 기존 테스트 변경만 test-diff에 남는 해당 커밋에 LEFTHOOK=0을 적용하고 사유를 커밋 메시지에 남긴다. 1·2차 커밋 모두 정상 훅에서 나머지 검사를 확인한 뒤 이 예외로 만들었다. 최종 검사도 승인된 test-diff 지적과 다른 검사의 결과를 구분해 기록한다. 전체 just check가 승인 예외 없이 이미 통과했다고 주장하지 않는다.


## 1차 리뷰 반영

새 Opus와 새 Codex 비판에서 중요2건을 확정했다. 반환 후 자식 소멸 대기를 판정 앞에 두어 검출력이 약해진 부분을 고쳤다. 정상 코드22.04초 통과, 같은 컴파일 가능한 러너만 종료하는 손변형은31.99초에 `nextest child survived deadline`으로 실패했다. 자식 해제용 파일은 테스트 판정 뒤 정리할 때만 쓰므로 나중 소멸이 판정을 바꾸지 않는다. PID 신호 정리 대신 파일을 통한 fixture 해제를 쓴다.

Slice는 대상 수가 조금 달라져도 빈 조각이 생긴다(4445개면127번,4481개면125~127번). 정상 빈 실행은 mutants.json=[]와 빈 결과 파일4개를 쓰고 outcomes.json·baseline을 만들지 않는다. 정확히 이 산출물과 단계의 성공 종료를 확인한 경우에만 대상 없음으로 성공 처리한다. 목록·결과 파일 유실, 모순된 결과, 실패·취소는 계속 실패다. 실제 빈 CLI 실행을 회귀로 추가했다. 128조각의 비중복·합집합은 빈 조각을 허용하더라도 그대로 유지된다.

분할 회귀는 xtask와 같은 no-config·cap-lints·nextest·명시 file 인자로 맞췄다. 129회 CLI 실행의158~381초 비용은 기록하고, 실제 전체128조각 증거를 유지하기로 했다. 현재 config/명시 방식 대상4452개가 같음은 비판에서 독립 확인됐다. 문서의 네 조각 설명은128로 바로잡았다. ADR 번호·초안 지적은 사용자 명시 지시에 따라 기각했다. 상세 평가와 측정은 docs/reviews/H-47.md에 있다.

수정 뒤 JavaScript 손변형은 임시 복사본에서26개를 node --check로 컴파일 가능한지 확인한 뒤 실제 실행했고 모두 검출됐다. 종료 신호 제거·러너만 kill하는 별도 Rust 변형도 실제 컴파일·실행해 검출했다. 아래 추가 표는1차 표의 뒤에 붙이는 수정 회차 증거다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 22 | 빈 목록 길이0 판정을 반대로 | H47 successful empty slices | 검출 |
| 23 | outcomes 부재 조건을 존재로 | H47 successful empty slices | 검출 |
| 24 | 빈 결과 파일 every를 some으로 | H47 successful empty slices | 검출 |
| 25 | 빈 결과 내용 비교를 반대로 | H47 successful empty slices | 검출 |
| 26 | 대상 없음 상태를 불완전으로 | H47 successful empty slices | 검출 |
| 27 | SIGINT 제거와 그룹 KILL을 러너만 kill로 교체 | deadline_stops_nextest_child_groups_and_preserves_unrelated_pid | 검출, exit101 |

## 2차 리뷰 반영

중요1건: 정상 빈 조각과 전체 목록0개는 다르다. 기존 설정 검사는 세 glob 문자열을 고정하지만 파일 매칭이나 변이 생성 수를 보장하지 않는다. 따라서 Everything/Sharded 실행은 실제 선택 인자로 분할 전 전체 catalog를 열거하고, name/file이 있는 비어 있지 않은 배열임을 확인한 뒤 target/mutants/catalog.json에 저장한다. 이전 catalog와 결과는 먼저 제거한다. 개별 빈 조각은 이 전체 목록 증거와 정상 빈 산출물을 모두 갖춘 때만 성공이다. 전체0개는 실제 CLI 회귀에서 거부되고 오래된 증거도 제거된다. 분할 회귀는 config 기반 전체 목록의 양수를 단언하고 실제 xtask 인자의 각 조각과 비중복·합집합을 대조한다. catalog 준비 비용도 변이 단계160분 안에 포함된다.

사소1건: 성공한 Fixture 정리에서 이미 종료된 숫자 PID를 다시 관찰하면 재사용된 PID를 기다릴 수 있다. 반환 직후 종료를 확인한 fixture는 Finished로 기록해 Drop에서 release나 PID 관찰을 하지 않는다. 살아 있는 무관한 PID를 넣은 새 회귀는 대기 시간 단언 없이 이를 검증한다. 실패 경로는 release 뒤 최대180초 정리를 유지하며 숫자 PID에 종료 신호를 보내지 않는다.

## 추가 조사: 훅 하위 프로세스

사용자가 지목한 commit_guard_refuses_unstaged_and_untracked_changes, commit_guard_refuses_content_that_lives_only_in_the_index, commit_hook_runs_the_check_and_rejects_violating_commits, branch_commits_may_change_their_own_tests_but_not_tests_from_master에는 자체 시간 제한이나 고정 sleep이 없고 동기 Command.output을 사용한다. 선택4개는 no-fail-fast로31.846초에 모두 통과했다. 원래 signal9를 보낸 주체는 보존 로그에서 확인하지 못했으며 nextest 정리 때문이라고 단정하지 않는다.

별도로 같은 테스트를 겹쳐 실행하면 공통 고정 fixture 경로를 두 번째 실행이 삭제하는 결함을 재현했다. 첫 실행을 just 진입 직전 ready/release 파일로 멈춘 뒤 두 번째 실행을 완료시키고 풀면 기존 첫 실행은 clean 단언 실패(exit101), 두 번째는0이었다. 새 fixture 보존 회귀도 이전 구현에서 실패했다. git_hooks 두 repository 생성 헬퍼만 기존 PrivateDirectory의 고유0700 경로를 사용하도록 바꿨다. 다른 테스트의 고정 경로 계약은 유지한다. 새 회귀는 통과했고 같은 조건 장벽 실험에서 두 실행 모두0이다. 네 기존 테스트의 단언·실행 시간 제한은 바꾸지 않았다. 이 충돌 재현은 원래 SIGKILL 재현과 구분한다.

3차 준비 중 xtask 전체 검사에서 공통 fresh_directory까지 고유화하면 고정 경로 단언이 깨지는 것을 발견해, 고유화를 위 두 훅 헬퍼로 한정했다. 검사 실패를 피해 기존 단언을 바꾸지 않았다.

## 최신 손 변형 실행

수정된 요약·정책의 JS 변형34개는 각각 mktemp 복사본에서 node --check와 실제 회귀를 실행해 모두 검출했다. 아래는 최신 실행의 각 변형이며 앞 표와 중복되는 번호는 최신 실행을 뜻한다. Rust 전체 catalog 검증을 생략하는 별도 변형도 실제 컴파일되고 전체0개 CLI 회귀에 검출됐다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| JS1 | `outcomes.total_mutants === planned.length` 변경 | H47 요약·정책 회귀 | 검출 |
| JS2 | `outcomes.total_mutants === planned.length` 변경 | H47 요약·정책 회귀 | 검출 |
| JS3 | `outcomes.total_mutants === planned.length` 변경 | H47 요약·정책 회귀 | 검출 |
| JS4 | `planned.length > 0` 변경 | H47 요약·정책 회귀 | 검출 |
| JS5 | `entry.scenario === "Baseline"` 변경 | H47 요약·정책 회귀 | 검출 |
| JS6 | `entry.summary === "Success"` 변경 | H47 요약·정책 회귀 | 검출 |
| JS7 | `&& entry.summary` 변경 | H47 요약·정책 회귀 | 검출 |
| JS8 | `state = "완주"` 변경 | H47 요약·정책 회귀 | 검출 |
| JS9 | `let state = "불완전"` 변경 | H47 요약·정책 회귀 | 검출 |
| JS10 | `state = "불완전"; }` 변경 | H47 요약·정책 회귀 | 검출 |
| JS11 | `outcome !== "success"` 변경 | H47 요약·정책 회귀 | 검출 |
| JS12 | `!["완주", "대상 없음"].includes(state)` 변경 | H47 요약·정책 회귀 | 검출 |
| JS13 | `/ /  !["완주", "대상 없음"].includes(state)` 변경 | H47 요약·정책 회귀 | 검출 |
| JS14 | `process.exitCode = 1` 변경 | H47 요약·정책 회귀 | 검출 |
| JS15 | `"missed.txt"` 변경 | H47 요약·정책 회귀 | 검출 |
| JS16 | `"timeout.txt"` 변경 | H47 요약·정책 회귀 | 검출 |
| JS17 | `process.env.MUTATION_SHARD` 변경 | H47 요약·정책 회귀 | 검출 |
| JS18 | `appendFileSync(process.env.GITHUB_STEP_SUMMARY, summary)` 변경 | H47 요약·정책 회귀 | 검출 |
| JS19 | `step["timeout-minutes"] === 160` 변경 | H47 요약·정책 회귀 | 검출 |
| JS20 | `step.name === "mutants without network"` 변경 | H47 요약·정책 회귀 | 검출 |
| JS21 | `job === "mutants"` 변경 | H47 요약·정책 회귀 | 검출 |
| JS22 | `planned.length === 0` 변경 | H47 요약·정책 회귀 | 검출 |
| JS23 | `!existsSync(join(directory, "outcomes.json"))` 변경 | H47 요약·정책 회귀 | 검출 |
| JS24 | `].every(` 변경 | H47 요약·정책 회귀 | 검출 |
| JS25 | `read(name).trim() === ""` 변경 | H47 요약·정책 회귀 | 검출 |
| JS26 | `state = "대상 없음"` 변경 | H47 요약·정책 회귀 | 검출 |
| JS27 | `catalog.length === 0` 변경 | H47 요약·정책 회귀 | 검출 |
| JS28 | `!Array.isArray(catalog)` 변경 | H47 요약·정책 회귀 | 검출 |
| JS29 | `!catalog.every` 변경 | H47 요약·정책 회귀 | 검출 |
| JS30 | `["name", "file"].every` 변경 | H47 요약·정책 회귀 | 검출 |
| JS31 | `typeof entry?.[field] === "string"` 변경 | H47 요약·정책 회귀 | 검출 |
| JS32 | `entry[field].length > 0` 변경 | H47 요약·정책 회귀 | 검출 |
| JS33 | `throw new Error("밤 전체 변이 목록이 없다");` 변경 | H47 요약·정책 회귀 | 검출 |
| JS34 | `read("../catalog.json")` 변경 | H47 요약·정책 회귀 | 검출 |
| Rust1 | catalog validate 호출 생략, 함수 참조만 유지 | nightly_rejects_an_empty_catalog_and_clears_previous_evidence | 컴파일 후 검출, nextest100 |
| Rust2 | SIGINT 제거와 그룹 KILL을 러너만 kill로 교체 | deadline_stops_nextest_child_groups_and_preserves_unrelated_pid | 컴파일 후 기존 자식 생존 단언으로 검출 |

3차 수정 정상 훅은 workspace1299/1299(127.107초), coverage945/945(88.977초), coverage-check 위반0이며 분할 실제129목록 회귀는178.597초에 통과했다. 전체 훅802.67초 뒤 test-diff에서 멈췄다. import 변화가 만든 부수 지적은 기존 import를 유지하고 고유root에서 기존 fresh_directory를 부르는 방식으로 제거한다. 남는 승인 변경은 기존 테스트2개와 훅 fixture 생성 헬퍼2개이며 단언은 보존한다. nextest --max-fail 도움말도 기본 :wait(실행 중 테스트 기다림), :immediate는 명시 옵션임을 확인했다. 원래 signal9를 일반 fail-fast 탓으로 단정하지 않는다. 필수 mutants-diff는 원기준8ba1f6a로 실행했으나 변이 실행 전 test-diff에서 중단(exit1)됐으므로 변이 통과로 세지 않는다. 실제 실행한 손 변형36개를 별도 보완 증거로 둔다.

import 정리 뒤 xtask 전체 대상 Clippy와 git_hooks60/60(24.301초)이 통과했다. test-diff를 다시 실행해 승인된 기존 테스트2건·fixture 헬퍼2건만 남는 것을 확인했다. disable-report·disable-report-since·hooks-installed도 통과했고 새 끄기는0건이다. 사용자 명시 예외를 이번 수정 커밋에 적용하고 사유를 커밋 메시지에 남긴다.
