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

새 대기 방식의 nextest 단독 회귀는22.465초에 PASS, LEAK 없음. CARGO=/bin/false에서도 미리 빌드한 테스트가 시작되고 종료 단언을 통과했다. 지정 PATH를 적용한 관련 xtask·git_hooks 묶음은74/74 통과(50.760초), 종료 회귀 자체는25.301초, LEAK 없음. 실행 전 같은 기계의 load averages는67.06/72.47/86.78이었다. 정확히 load100을 재현했다고 주장하지 않는다. 첫 묶음 실행에서는 지정 pnpm PATH가 빠져 corepack pnpm 모듈을 못 찾았으며, 올바른 PATH로 다시 실행해 모두 통과했다. 이는 시간 대기 실패가 아니므로 git_hooks 테스트는 수정하지 않았다. xtask 전체 대상 Clippy도 통과했다. 정상 훅 결과는 후속 기록으로 갱신한다.


## 커밋 검사와 승인 예외

정상 커밋 훅의 전체 검사에서1294/1294 workspace 테스트와945/945 커버리지 테스트가 통과했고 coverage-check 위반은 없다. 최종 test-diff는 승인된 두 Rust 기존 테스트의 본문 변경만 지적한다: shard_selection_accepts_only_fixed_partition의 정확한 분할 목록 갱신과 추가 승인된 deadline_stops_nextest_child_groups_and_preserves_unrelated_pid의 대기 방식 변경. 새 import의 shadowing 두 건은 완전한 경로 표기로 제거하고 test-diff를 다시 실행해 위 두 건만 남는 것을 확인했다. 이 표기 변경 뒤 xtask 전체 대상 Clippy도 통과했다.

나머지 전체 검사 단계는 통과했고, test-diff 뒤 단계인 disable-report·disable-report-since·hooks-installed도 따로 통과했다. 새 끄기는0건이다. 사용자가 지시한 승인 예외에 따라 이 승인된 기존 테스트 변경을 담는 구현 커밋 하나만 LEFTHOOK=0으로 만들고 사유를 커밋 메시지에 남긴다. 리뷰·기록 정리 뒤 최종 커밋은 정상 훅의 전체 검사를 통과시킨다. 전체 just check가 승인 예외 없이 이미 통과했다고 주장하지 않는다.


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
