# ADR 0086: H-47 밤 변이 실행: 매일 변경분, 주 1회 전체

- 상태: 확정 (2026-11-16)


## 현행 결정: 일일 변경분과 주간 전체

메인 결정으로 매일은 지난24시간의 master 변경분을 선택량에 따라1~8작업으로 검사하고 전체는 주1회128분할로 실행한다. 일일 cron은18:00 UTC(매일03:00 KST), 주간은일요일14:00 UTC(일요일23:00 KST)다. workflow_dispatch의 mode는 daily/full이며 기본daily다. 두 모드 모두 master 이벤트의 고정 github.sha를 checkout하고 ref가 refs/heads/master인지 검증한다. 수동 실행도 master에서 요청해야 하며 다른 브랜치는 명시적으로 실패한다. 128개 작업이 서로 다른 master 끝을 읽는 경쟁을 막는다. 정상 macOS/Linux 검사는 기존 일일 cron에서만 유지하고 주간·수동 실행에서는 중복하지 않는다.

일일은 Git의 master 첫 부모 이력에서 고정된 예약 기준24시간 전의 마지막 커밋을 고른다. checkout은fetch-depth0이고 얕은 복제면 fetch --unshallow origin을 먼저 수행한다. 가져오기 실패는 성공이나 빈 구간으로 바꾸지 않는다. 과거 커밋이 없는 저장소 첫24시간은 initial로 표시해 빈 트리부터 모든 현재 대상 소스를 고른다. 기준HEAD와 같거나 선택 변이가0이면 변경 없음이다. 기존 diff_scope·diff_patch·list_changed_mutants를 재사용하여 대상 src와 diff 영역의 교집합을 검사한다.

일일 계획 단계는15분·작업은30분이며 실제 선택 변이100개당 한 작업을 만들고1~8개로 제한한다. 일일 변이 단계330분·작업360분 제한이다. 주간 전체는160분·180분을 유지한다. 작업 브랜치 mutants-diff의20분·test-diff 선행 검사는 유지한다. 일일은 이미 master에 반영된 승인 테스트 변경을 다시 거부하지 않으며 선택 로직과 missed/timeout evaluate를 재사용한다. 주간 전체·일일 모두 기본256회, 미검출·시간 초과 실패, always 요약을 유지한다. 일일의 catalog는 실제 diff 선택 목록이며 빈 목록은 유효한 기준·HEAD 영수증과 빈 결과가 함께 있을 때만 변경 없음이다. 선택 목록이 양수인데 계획이 비면 실패한다. 아래 이전 회차의 전체 매일 실행 계획은 이 결정으로 대체된다.

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

중요1건: 정상 빈 조각과 전체 목록0개는 다르다. 기존 설정 검사는 세 glob 문자열을 고정하지만 파일 매칭이나 변이 생성 수를 보장하지 않는다. 따라서 Everything/Sharded 실행은 실제 선택 인자로 분할 전 전체 catalog를 열거하고, name/file이 있는 비어 있지 않은 배열임을 확인한 뒤 target/mutants/catalog.json에 저장한다. 이전 catalog와 결과는 먼저 제거한다. 개별 빈 조각은 이 전체 목록 증거와 정상 빈 산출물을 모두 갖춘 때만 성공이다. 전체0개는 실제 CLI 회귀에서 거부되고 오래된 증거도 제거된다. 분할 회귀는 config 기반 전체 목록의 양수를 단언하고 현행 xtask와 같은 인자를 JS에서 별도로 구성한 각 조각과 비중복·합집합을 대조한다. catalog 준비 비용도 변이 단계160분 안에 포함된다.

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

## 3차 리뷰 최종 인계

Opus·Codex 각3회 완료. 기존 중요 결함3건은 해소됐으나 훅4테스트의 원래 SIGKILL 발신 주체·원인 미확인이 중요1건으로 남는다. 이 항목을 완료로 보고하지 않는다. 단일 nextest 안의 네 fixture 이름은 구별되므로 별개 실행 충돌의 재현·고유화가 원래 실패의 원인 규명을 대체하지 않는다. 자체 timeout은 없고 기본 nextest max-fail은 :wait, 기본 slow 경고의 terminate_after는 None이다. 다음 발생 시 메인은 실패한 원본 전체 로그와 실제 명령·nextest 프로필·PID/PPID/PGID·부모 종료 상태·종료 신호 시각을 보존해 원인을 조사해야 한다. 현재 기록만으로 nextest·외부 종료·자원 문제 중 하나를 확정하지 않는다.

catalog·실제 실행의 인자 조립은 별도이며 현행 선택 필터는 일치한다. JS 분할 회귀도 Rust 명령 생성기를 호출하지 않고 같은 인자를 독립 구성한다. 실행 쪽에만 exclude가 추가되는 미래 변경에는 검증 공백이 있으므로 필터 변경 때 공통화 또는 계획 목록 대조를 추가하는 예방적 보강으로 인계한다. 현행 전체0개 차단은 실제 CLI로 양쪽 리뷰에서 확인됐다.

INT를 그룹 대신 러너 PID에만 보내는 컴파일 가능한 변형은 정상22.69초·변형23.14초에 둘 다 통과했고 nextest가 자식을 정리했다. 이는 요구된 결과 계약을 만족하므로 결함으로 세지 않으며, 종료 회귀가 신호 구현의 모든 변형을 잡는다고 주장하지 않는다. fixture 폴더는 기존1시간 stale 지연 정리이며 무한 누적은 아니다.

master0739990f8eaa8db529dd13b3e6e7b6d01809140b를 --no-ff로 받으며 environment.rs 충돌은 사용자 지시에 따라 조건 대기를 유지하고 임시60초·900초 값을 버렸다. 생성 설정·해시는 공식 생성기로 병합한다. master에서 받은 tasks/progress는 그대로 보존하며 구현자가 편집하지 않는다.

## 최종 병합 검사

master0739990을 합친 소스의 실제 catalog는4471개다. ceil(4471/128)=35로 최대 조각 크기는 같고,35×172초÷2=50.167분, 두 배와 준비20분을 더하면120.333분이다. 단계160분·작업180분의 추정 근거는 유지한다. 실제128조각 합집합·비중복 회귀는105.399초에 통과했다. 원격 미검증, 다음 밤 실행에서 확인한다.

정상 병합 커밋 훅718.38초: workspace1319/1319(161.248초), coverage965/965(56.557초), coverage-check 위반0. 종료 회귀24.531초와 지목된 훅4테스트도 모두 통과했다. 전체 검사 명령은 마지막 test-diff의 승인4건(정확 분할·deadline 본문2건, fixture 생성 헬퍼2건) 때문에 exit1이므로 전체 just check exit0으로 보고하지 않는다. 앞 단계의 TS·정책·정적검사·Clippy 등은 통과했다. 사용자 명시 예외를 병합 커밋에도 적용할 때 사유를 커밋 메시지에 남긴다. 앞선 구현·수정3커밋에도 각각 동일 예외를 적용했다.

필수 변경분 변이 명령은 원기준8ba1f6a로 실제 실행했지만 test-diff에서 중단됐고 변이 통과가 아니다. 병합 커밋을 만들기 전 master0739990으로 실행한 명령은 아직 HEAD의 조상이 아니라는 선행 검증에서 중단됐다. 병합을 마친 뒤 같은 기준으로 다시 실행한 최종 결과는 보고서와 실행 로그에 남긴다. 승인4건 외에 검사 규칙을 느슨하게 하거나 새 끄기를 추가하지 않았다.

최종 상태는 막힘이다. 코드로 확인한 기존 중요 결함3건은 수정했으나 추가 조사 항목인 원래 SIGKILL의 발신 주체·원인에 대한 중요 증거 공백1건이 최대3회 리뷰 뒤에도 남는다. 고유 fixture 충돌 수정이나 한 번의 전체 검사 통과를 그 원인의 해결 증거로 세지 않는다.

## 미해결: SIGKILL 조사 기록 종료

메인은 남은 중요1건을 원인 미확인 상태로 기록하고 닫았으며 막힘을 해제했다. 네 훅 테스트에는 자체 시간 제한이 없고 nextest 기본 실패 중단은 :wait다. 별개 실행 간 fixture 삭제 충돌은 고쳤다. 그 수정이 원래 SIGKILL을 해결했다는 증거는 없으며 그 뒤 재발 여부는 관찰 대상이다. 재발하면 전체 로그, 실제 실행 명령, PID/PPID/PGID, 종료 시각을 보존한다. 이전 회차의 막힘 판정은 이 결정으로 종료한다.

## 일일·주간 설계 회차 검증

새 일정·수동 선택 회귀는 변경 전 실패했고 변경 후 통과했다. 실제 Git 저장소로 조용한24시간·첫24시간·얕은clone 복구·fetch 실패·HEAD 부재·master 첫 부모와 side branch 날짜 구분·24시간 경계 양쪽을 검증했다. 실제 Cargo fixture는 루트 초기 소스의 변이를 검사하고 약한 테스트의 미검출을 실패 findings로 남겼다. 기본256 프로필과 기존20분 작업 브랜치 제한은 유지한다.

첫 정상 훅은 새 justfile 레시피의 보호 기대값 등록 누락으로 check-ts에서 중단됐다. harness/check-list/justfile.json에 mutants-daily 한 레시피만 추가했으며 기존 기대값은 그대로다. 해당 검사와 새 일정·범위 회귀17/17이 통과했다. 이 오류는 test-diff 예외로 넘기지 않고 수정했다.

새 일정·요약 코드의 손 변형22개를 별도 mktemp 복사본에서 node --check로 컴파일 확인한 뒤 실제 회귀로 실행해 모두 검출했다. 아래 표는 이번 회차 실행만 기록한다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| S1 | "--first-parent", →  | H47 daily range follows master first parents and fails closed without HEAD | 컴파일 후 검출 |
| S2 | - 24 * 60 * 60 * 1000 → + 24 * 60 * 60 * 1000 | H47 daily range covers quiet windows initial history and shallow clones | 컴파일 후 검출 |
| S3 | - 24 * 60 * 60 * 1000 → - 0 * 60 * 60 * 1000 | H47 daily range covers quiet windows initial history and shallow clones | 컴파일 후 검출 |
| S4 | === "true" → === "false" | H47 daily range covers quiet windows initial history and shallow clones | 컴파일 후 검출 |
| S5 | "--unshallow" → "--depth=1" | H47 daily range covers quiet windows initial history and shallow clones | 컴파일 후 검출 |
| S6 | run.status !== 0 → run.status === 0 | H47 daily range covers quiet windows initial history and shallow clones | 컴파일 후 검출 |
| S7 | --before=${cutoff} → --after=${cutoff} | H47 daily range covers quiet windows initial history and shallow clones | 컴파일 후 검출 |
| S8 | ]) \|\| "initial" → ]) \|\| "HEAD" | H47 daily range covers quiet windows initial history and shallow clones | 컴파일 후 검출 |
| S9 | process.env.MUTATION_MODE === "daily" → false | H47 daily summary requires range evidence and preserves cancellation and completion failures | 컴파일 후 검출 |
| S10 | process.env.MUTATION_MODE === "daily" → true | H47 completion requires a successful baseline and every planned mutant | 컴파일 후 검출 |
| S11 | read("../daily.json") → read("../missing-daily.json") | H47 daily summary requires range evidence and preserves cancellation and completion failures | 컴파일 후 검출 |
| S12 | !validCommit(receipt.head) → !validCommit(receipt.base) | H47 daily summary requires range evidence and preserves cancellation and completion failures | 컴파일 후 검출 |
| S13 | receipt.base === "initial" → receipt.base === "never" | H47 daily summary requires range evidence and preserves cancellation and completion failures | 컴파일 후 검출 |
| S14 | if (daily) { → if (false) { | H47 daily summary requires range evidence and preserves cancellation and completion failures | 컴파일 후 검출 |
| S15 | outcome !== "success" → outcome === "success" | H47 completion requires a successful baseline and every planned mutant | 컴파일 후 검출 |
| S16 | (!daily && catalog.length === 0) → false | H47 missing empty or malformed global catalogs never validate an empty shard | 컴파일 후 검출 |
| S17 | (!daily \|\| catalog.length === 0) → true | H47 daily summary requires range evidence and preserves cancellation and completion failures | 컴파일 후 검출 |
| S18 | (!daily \|\| planned.length === catalog.length) → (!daily \|\| planned.length !== catalog.length) | H47 daily summary requires range evidence and preserves cancellation and completion failures | 컴파일 후 검출 |
| S19 | state = daily ? "변경 없음" : "대상 없음" → state = daily ? "대상 없음" : "변경 없음" | H47 successful empty slices have an explicit empty result without baseline | 컴파일 후 검출 |
| S20 | entry.summary === "Success" → entry.summary !== "Success" | H47 completion requires a successful baseline and every planned mutant | 컴파일 후 검출 |
| S21 | entry.scenario === "Baseline" → entry.scenario !== "Baseline" | H47 completion requires a successful baseline and every planned mutant | 컴파일 후 검출 |
| S22 | process.exitCode = 1 → process.exitCode = 0 | H47 cancellation and missing results remain visible and fail | 컴파일 후 검출 |

전체 검사 재시도에서 ci-plumbing의 예전 일정 하나만 기대하는 단언을 발견해 새 일정2개와 수동 입력의 정확한 값으로 갱신했다. 기존 단언은 삭제하지 않았다. 추가 검증에서 수동 전체128조각도 같은 commit을 써야 하므로 checkout을 움직이는 master 참조 대신 실행 이벤트SHA로 고정하고 master ref·HEAD SHA 일치를 검증한다. 이 변경은 정상/잘못된 ref/다른 SHA/누락 SHA 회귀로 확인한다.

경계·스냅샷 보강: 정상24시간은 통과하고23시간·25시간 변형은 각각 실패했다. master ref·SHA 검증의 추가5개 변형도 실제 컴파일·실행 후 모두 검출했다. 이 회차 손 변형은 합계29개이며 전부 실제 실행했다. 최신 CI 회귀 전체81/81, 하네스 ESLint 통과.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| S23 | 24시간 → 23시간 | H47 daily cutoff includes exactly the prior 24 hours | 컴파일 후 검출 |
| S24 | 24시간 → 25시간 | H47 daily cutoff includes exactly the prior 24 hours | 컴파일 후 검출 |
| S25 | ref !== "refs/heads/master" → ref === "refs/heads/master" | H47 manual mutation runs require the pinned master snapshot | 컴파일 후 검출(exit1) |
| S26 | !/^[0-9a-f]{40}$/u.test(sha) → !/^[0-9a-f]{39}$/u.test(sha) | H47 manual mutation runs require the pinned master snapshot | 컴파일 후 검출(exit1) |
| S27 | git(root, ["rev-parse", "HEAD"]) !== sha → git(root, ["rev-parse", "HEAD"]) === sha | H47 manual mutation runs require the pinned master snapshot | 컴파일 후 검출(exit1) |
| S28 | ref !== "refs/heads/master" \|\| → false \|\| | H47 manual mutation runs require the pinned master snapshot | 컴파일 후 검출(exit1) |
| S29 | throw new Error("변이 실행은 master의 고정된 실행 SHA가 필요하다"); → ; | H47 manual mutation runs require the pinned master snapshot | 컴파일 후 검출(exit1) |

4회차 정상 통합 검사: master580a19874ee4ad32c21c4c3f83a0a4142b3a4858 기준 workspace1332/1332(114.931초), coverage975/975(86.911초), coverage-check 위반0. 실제128분할 목록 회귀81.443초 통과. 전체 정상 훅631.62초 뒤 승인된 test-diff 정확4건(기존 정확분할·deadline 테스트2건, 훅 fixture 헬퍼2건)에서 exit1이었다. 이4건 외 문제는 없고 후속 disable-report·disable-report-since·hooks-installed도 통과했다. 새 끄기0. 지시문의 해당 커밋 예외를 적용하고 사유를 커밋 메시지에 남긴다. 전체 명령이 exit0이었다고 표현하지 않는다.

## 단일 Opus 리뷰 반영

[일일·주간 Opus 리뷰](t3-thread://v2/6d045176-96ef-48cf-b853-22f631ce6b65/thread:delegated-task:command%3Amcp%3Acb0ccb1b-e346-4f2a-b148-260c0dd56a5d%3Adelegate-task%3A951635ea-8a33-47fb-ba55-a28b6aeecfa1)는 중요1·보통1·낮음1을 지적했다. 추가 Opus는 띄우지 않는다.

중요1은 수용해 적응 분할을 구현했다. 실제 CLI로 edd57f84fc68b66a417291424006f6f463b67938→현재 대상 소스에서936개, c1b3a5992a3047649d31f56bae8539d1c62f1287→현재 대상 소스에서1431개를 독립 재열거했다. 각각8개의 실제 CLI 목록은117개씩8개,179개씩7개와178개이며 비중복과 전체 합집합을 모두 확인했다. 변이는 실행하지 않은 목록 실측이다.

분할만으로는1431개의 느린 경로가160분에 들어오지 않으므로 일일에만 단계330분·작업360분을 적용한다. 179×172초÷2=256.567분, 변이 비용20% 여유와 baseline10분을 더하면317.88분으로330분 안이다. 준비·요약은 작업360분과의30분 차이에 둔다. 936개는 조각117개이므로167.7분+baseline으로 추정한다. GitHub의 [단계 제한 최대360분](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idstepstimeout-minutes) 안이다. 다른 작업의 기존180분 정책과 주간160/180분은 유지하며 일일 작업만 정확히 예외를 둔다. 이 계산은 원격 실측 완주 보장이 아니다. 8조각 상한보다 더 큰 변경일은 기한을 넘으면 실패·불완전 요약을 남기며 대상을 버리거나 검사 강도를 낮추지 않는다.

보통2 중 설치·큐 지연의 틈은 수정했다. 계획 작업은 [자신의 실행 메타데이터](https://docs.github.com/en/rest/actions/workflow-runs#get-a-workflow-run)의 created_at을 읽어 일일18:00 UTC 예약 기준을 고정한다. worker는 그때 정한 base와 고정 SHA를 모두 재사용하므로 도구 설치나 worker 큐 대기에 따라 기준이 움직이지 않는다. 수동 실행은 생성 시각 그대로다. 조회 실패는 현재 시각으로 대체하지 않고 실패한다. Actions 읽기 권한은 이 메타데이터 작업에만 부여한다.

늦게 push한 옛 날짜 커밋·커밋 시계 역행·누락된 예약 실행은 main이 지정한 Git 커밋 날짜 기반24시간 선택의 한계로 남는다. 마지막 성공 SHA 방식으로 몰래 바꾸거나24시간을 임의로 늘리지 않는다. 이때 변경 없음은 해당 커밋 시각 창의 선택 변이0개를 뜻하며 최근 push 전체 검사 완료를 보장하지 않는다. 주간 전체가 최대7일 뒤 보완한다.

낮음3은 수정했다. 일반 changes/macOS/Linux/notify는 기존 일일 cron에만 실행되고 주간·수동 변이에서 중복되지 않는다. 일일 plan과worker는 일반 검사 결과에 의존하지 않으며 plan 실패도 변이 알림 대상으로 포함한다. 일일 실제 계획과 catalog의 정확한 조각별 name/file 목록을 대조해 다른 필터로 비어 버리는 실행도 성공으로 숨기지 않는다.

### 적응 분할 수정의 손 변형

별도 mktemp 복사본에서 계획·요약의24개 JS 변형을 node --check 뒤 실제 h47-plan 회귀로 실행했다. 처음에 요약의 선택 수를 조각 수로 바꾼 변형이 생존해 서로 다른5/1 값을 단언하는 회귀를 보강했고, 전체24개를 재실행해 모두 검출했다. 실제 Rust valid_shard 함수와 기존 해당 테스트를 그대로 추출해 rustc로 컴파일한 정상은0, 상한9·마지막 인덱스 포함·비교 반전3변형은 각각101로 실패했다. 아래27개는 모두 컴파일 가능한 실제 실행이며 이전 회차29개와 구분한다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| A1 | Math.min(8, → Math.min(9, | H47 adaptive daily shards are bounded complete nonoverlapping and budget actual review windows | 검출 |
| A2 | Math.max(1, → Math.max(0, | H47 adaptive daily shards are bounded complete nonoverlapping and budget actual review windows | 검출 |
| A3 | Math.ceil(count / 100) → Math.floor(count / 100) | H47 adaptive daily shards are bounded complete nonoverlapping and budget actual review windows | 검출 |
| A4 | count / 100 → count / 1000 | H47 adaptive daily shards are bounded complete nonoverlapping and budget actual review windows | 검출 |
| A5 | !Number.isSafeInteger(count) → Number.isSafeInteger(count) | H47 adaptive daily shards are bounded complete nonoverlapping and budget actual review windows | 검출 |
| A6 | ${index}/${total} → ${index + 1}/${total} | H47 adaptive daily shards are bounded complete nonoverlapping and budget actual review windows | 검출 |
| A7 | Math.ceil(catalog.length / total) → Math.floor(catalog.length / total) | H47 adaptive daily shards are bounded complete nonoverlapping and budget actual review windows | 검출 |
| A8 | index * chunk, → (index + 1) * chunk, | H47 adaptive daily shards are bounded complete nonoverlapping and budget actual review windows | 검출 |
| A9 | index >= total → index > total | H47 adaptive daily shards are bounded complete nonoverlapping and budget actual review windows | 검출 |
| A10 | [1-8] → [1-9] | H47 adaptive daily shards are bounded complete nonoverlapping and budget actual review windows | 검출 |
| A11 | date.getUTCDate(), 18 → date.getUTCDate(), 17 | H47 scheduled anchors ignore installation queue delay and preserve manual creation time | 검출 |
| A12 | anchor > date → anchor < date | H47 scheduled anchors ignore installation queue delay and preserve manual creation time | 검출 |
| A13 | anchor.getUTCDate() - 1 → anchor.getUTCDate() + 1 | H47 scheduled anchors ignore installation queue delay and preserve manual creation time | 검출 |
| A14 | event === "workflow_dispatch" → event === "schedule" | H47 scheduled anchors ignore installation queue delay and preserve manual creation time | 검출 |
| A15 | typeof createdAt !== "string" → typeof createdAt !== "number" | H47 scheduled anchors ignore installation queue delay and preserve manual creation time | 검출 |
| A16 | ".created_at" → ".run_started_at" | H47 planner reads run metadata once emits a bounded matrix and fails closed | 검출 |
| A17 | result.status !== 0 → result.status === 0 | H47 planner reads run metadata once emits a bounded matrix and fails closed | 검출 |
| A18 | run.status !== 0 → run.status === 0 | H47 planner reads run metadata once emits a bounded matrix and fails closed | 검출 |
| A19 | count: catalog.length → count: shards.length | H47 planner reads run metadata once emits a bounded matrix and fails closed | 검출 |
| A20 | "target/mutants/plan.json" → "target/mutants/wrong-plan.json" | H47 planner reads run metadata once emits a bounded matrix and fails closed | 검출 |
| A21 | sliceFor(catalog, receipt.shard) → catalog | H47 plan summaries fail closed and daily shards validate exact planned identities | 검출 |
| A22 | receipt.shard !== process.env.MUTATION_SHARD → receipt.shard === process.env.MUTATION_SHARD | H47 plan summaries fail closed and daily shards validate exact planned identities | 검출 |
| A23 | JSON.stringify(shardsFor(catalog.length)) → JSON.stringify([]) | H47 plan summaries fail closed and daily shards validate exact planned identities | 검출 |
| A24 | 선택 변이: ${catalog.length} → 선택 변이: ${plan.shards.length} | H47 plan summaries fail closed and daily shards validate exact planned identities | 검출 |
| R1 | valid_shard 상한8→9 | daily_shards_are_canonical_and_never_exceed_eight | 컴파일 후 검출(exit101) |
| R2 | 인덱스 끝 포함 | daily_shards_are_canonical_and_never_exceed_eight | 컴파일 후 검출(exit101) |
| R3 | 조각 문자열 비교 반전 | daily_shards_are_canonical_and_never_exceed_eight | 컴파일 후 검출(exit101) |

Opus 수정 중 master가 D-42·DS-06을 포함한 ecb6e5a73b3fe5a394d55c0803bb203ad984cdf6로 전진했다. 이전 기준으로 진행하던 본인 검사 그룹만 확인 후 SIGINT로 중단했고, 중단된 bootstrap 임시 폴더만 정리했다. 이를 통과로 세지 않는다. 최신 master를 --no-ff로 추가 병합해 받은 도메인·차트·문서 변경을 보존하고 다시 통합 검사한다.

### 최신 통합과 검증 한계

ecb6e5a73b3fe5a394d55c0803bb203ad984cdf6 통합 커밋8668833630a6f5a05b62029d3a8ccb9e1d0a5c37의 정상 훅에서 workspace1347/1347, coverage987/987·위반0, CI87/87이 통과했다. 전체 명령은 승인된 test-diff 정확4건에서 exit1이었다. 후속 검사도 통과한 뒤 사용자 지정 커밋 예외를 적용했고 새 끄기는0건이다. 필수 mutants-diff도 같은 승인4건의 선행 검사에서 멈췄으며 실제 변경분 변이 통과를 주장하지 않는다.

이번 설계는 Opus1회와 새 Codex 비판1회를 거쳤다. 비판자는 일일 용량·일반CI 중복 지적의 해소와 예약 시각 고정을 확인했고 새 중요 코드 결함은 없었다. Node35개·Rust5개 및 실제 작은 fixture의 분할·생존 실패를 확인했다. 원격 일일1~8조각과 주간128조각의 실제 소요·완주, 계획15분 예산, 메타데이터 API 권한은 병합 뒤 각각 다음 실행에서 확인한다.

cancelled 입력의 실패 요약은 로컬로 검증했으나 작업 전체 강제 취소나 러너 유실에서 always 단계가 실제 실행되는지는 원격 미검증이다. 알림은 기존 failure() 조건이며 취소만 발생한 실행에 별도 알림을 보장하지 않는다. 늦은 push·옛 committer date·시계 역행·예약 누락과 하루 이상 지연은24시간 커밋 날짜 방식의 한계로 유지한다. 주간 전체가 정상 실행되면 그때 보완하며 마지막 성공 SHA부터 복구하는 기능을 주장하지 않는다.

최종 상태 확인에서 master가735ea38189edbf15d34238dd14ac49bb02c9e94f로 전진해 D-39·D-40·D-41도 추가 통합한다. H-47 구현은 리뷰 대상8668833 그대로이며 들어온 도메인 변경은 보존한다. 이전 실제936/1431개 열거와4471개 전체 목록은 각각 기록한 당시 소스의 관측값이다. 추가 통합 뒤 전체 검사와 승인 test-diff 건수는 새 master 기준으로 확인한다.

735ea381 통합 정상 훅: workspace1403/1403, coverage1043/1043·위반0, CI87/87, 실제128분할 합집합·비중복 회귀98.558초 통과. 전체637.42초 뒤 승인된 test-diff 정확4건에서만 exit1이었다. 후속 검사도 통과했고 새 끄기0건이며 사용자 지정 예외를 병합 커밋에 한정 적용한다. 받은 master의 tasks/progress와 제품 코드는 보존한다.
