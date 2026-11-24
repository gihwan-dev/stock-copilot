# ADR H-63 초안: 변경분 변이의 플랫폼 분리 및 조각 병렬 실행

- 상태: 제안
- 날짜: 2026-11-24
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-63 (H-59·H-62 후속)
- 반영한 문서: `.github/workflows/{ci,nightly}.yml`, `crates/xtask/src/{mutants_check,test_diff,nextest_list}*`, `harness/ci/{diff-mutants-plan,diff-mutants-shard,diff-mutants-aggregate,remote-check}.mjs`, `harness/ci/workflow-policy/{check.mjs,policy.json}`, `justfile`, `docs/harness.md`, `docs/workflow.md`

## 맥락

D-54 병합에서 `domain` 선택 변이 130건을 단일 `macos-15` 러너(`macos` / `macos-branch` 120분 예산)로 순차 실행하다가 약 40건 시점에서 시간 예산 초과(`budget`)로 실패했다. 또한 다음 세 가지 제약이 동시에 걸려 있다.

1. **시간·병렬성**: 큰 `domain`·`application`·`order-execution` 변경(100~160건 이상)을 GitHub Actions 단일 작업 예산 안에 끝내려면 여러 실행기로 나눠 병렬로 돌리고 결과를 하나의 판정으로 합쳐야 한다.
2. **H-59 macOS 러너 점유 상한**: macOS 러너 동시 실행은 상한 2를 지켜야 하며, `store-sqlite`만 macOS CommonCrypto(`harness/ci/linux/exclusions.json`)에 묶여 있고 나머지 변이 대상 크레이트(`domain`, `application`+`testkit-ports`, `order-execution`)는 플랫폼 중립이다.
3. **H-62 승인 묶음 연계**: 승인된 기존 테스트 변경이 있는 병합 커밋에서 선행 `test-diff`가 H-62 승인 묶음을 통과시키지 못하면 변이가 시작조차 되지 않는다.

## 실측

| 항목 | 명령 | 결과 |
|---|---|---|
| Rust 플랫폼 분리·조각 분할·H-62 묶음 선행 연계 실측 | `cargo nextest run --locked -p xtask mutants_check::h63_tests` | 플랫폼 분리(`domain`→Linux, `store-sqlite`→macOS), H-62 승인 묶음 통과 및 변조 거부, 실제 `cargo mutants` 기준 Linux 조각 + macOS 조각 합집합 = 전체 카탈로그 일치 확인 |
| Node 계획·조각 요약·최종 합산·정책·원격 검사 테스트 | `node --test harness/ci/tests/h63-diff-mutants.test.mjs harness/ci/tests/workflow-policy.test.mjs harness/ci/tests/h52-remote-check.test.mjs` | 조각 누락·실패·시간 초과·취소·건너뜀·중복·비일치·상한 초과·대기열 삭제 모두 실패(닫힌 실패) 확인 |
| 원격 GitHub Actions 병렬 완주 실측 | 병합 뒤 원격 `ci.yml` 실행 | 브랜치 푸시·병합 후 원격 CI에서 실제 소요 시간 확인 필요(이 기기에는 Docker가 없음) |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| `ci.yml`에서 변경분 변이를 `macos`/`macos-branch` 본체에서 분리해 `diff-mutants-plan` → (`diff-mutants-linux`, `diff-mutants-macos`) → `diff-mutants-verdict` 4작업 파이프라인으로 구성한다 | `just check`와 변이 실행을 분리해 `macos` 120분 예산 고갈을 없애고 여러 러너가 동시에 조각을 처리한다 | 단일 `macos-15` 작업 안에서 `diff-mutants.mjs` 직렬 분할 유지(D-54 130건 완주 불가) | H-63 완료 기준 |
| `harness/ci/linux/exclusions.json`을 단일 기준으로 삼아 플랫폼 중립 크레이트(`domain`, `application`+`testkit-ports`, `order-execution`) 변이는 `ubuntu-24.04-arm`(`max-parallel: 4`, `offline.mjs` 네트워크 격리)에서 실행하고, macOS 전용 크레이트(`store-sqlite`)만 `macos-15`(`max-parallel: 2`)에서 실행한다. CI `diff-mutants-macos`와 야간 `nightly.yml`의 `mutants`·`mutants-daily`는 모두 워크플로 이름 접두사가 없는 공통 2슬롯 `concurrency` 그룹(`macos-mutants-slot-{0,1}`, `queue: "max"`, `cancel-in-progress: false`)을 공유하며 `workflow-policy/check.mjs`가 이 교차 워크플로 일치를 강제한다 | 대부분의 변경(`domain`/`application`/`order-execution`)이 macOS 러너를 0분만 쓰면서도 4배 병렬로 빠르게 끝난다. `store-sqlite` 변경과 야간 변이가 겹쳐도 계정 전체 macOS 변이 동시 점유가 H-59(ADR 0156) 상한 2를 넘지 않는다 | `ci.yml`과 `nightly.yml`에 `${{ github.workflow }}`를 붙여 별도 그룹을 두기(2+2=4점유로 계정 한도 5 중 검사 자리 침범), 또는 모든 변이를 macOS에서만 실행 | H-63·H-59·ADR 0156·1회차 리뷰 지적 3 |
| 조각 수는 플랫폼별 선택 변이 수에 따라 `MUTANTS_PER_SHARD = 20`, `MAX_SHARDS = 8` (`0/N..(N-1)/N`)로 계산하고, 선택 변이가 0개인 플랫폼 작업은 건너뛴다 | D-54(130건) 기준 Linux 7조각(조각당 약 19건)으로 `max-parallel: 4` 2파에 여유 있게 완주하며, 문서·TS·단일 플랫폼 변경에서 불필요한 러너를 띄우지 않는다 | 항상 고정 4조각 또는 8조각 실행(소규모 변경에 러너 낭비) | H-63 완료 기준 |
| 각 조각 작업 끝에 `if: always()` 단계(`diff-mutants-shard.mjs summary`)를 두어 `missed.txt`, `timeout.txt`, `caught.txt`, `unviable.txt`를 표준 출력과 `$GITHUB_STEP_SUMMARY`에 남기고, 영수증(`diff-shard.json`)·카탈로그·조각 일치·Baseline 성공·시간 초과 0건을 검증해 불완전·실패·시간 초과 시 0이 아닌 종료를 낸다 | 조각이 실패·시간 초과·취소되어도 놓친 변이와 시간 초과 증거가 로그와 요약에 영구 보존되며, 불완전한 조각이 성공으로 통과할 수 없다 | `cargo mutants` 종료 코드만 보고 요약 생략 | H-63 완료 기준 |
| 최종 판정 작업(`diff-mutants-verdict`, `if: always() && github.event_name == 'push'`, 권한 `contents: read, actions: read`)이 `diff-mutants-aggregate.mjs`로 GitHub Actions Jobs API(페이지네이션 전수 조회)를 읽어 `diff-mutants-plan`과 계획된 모든 `diff-mutants-linux (<shard>)`·`diff-mutants-macos (<shard>)` 작업 및 필수 단계(`diff mutants shard`, `diff mutants summary`)의 완료·성공을 닫힌 실패로 검증한다. 선택 변이가 0개여서 `if`로 건너뛴 플랫폼 작업은 GitHub API가 돌려주는 `name: "diff-mutants-linux"\|"diff-mutants-macos"`, `status: "completed"`, `conclusion: "skipped"`, `steps: []`(SHA·회차 일치, 1건 이하, 같은 플랫폼 조각과 공존 금지)만 허용하고 그 외 모든 변형을 거부한다. `notify.needs`는 `diff-mutants-verdict`에 의존해 최종 판정 실패 시 알림을 보내고, `remote-check.mjs`는 동일한 조각·건너뜀·`run_attempt` 일치 규칙을 독립적으로 강제한다 | 실제 GitHub Actions `/jobs` API의 건너뛴 매트릭스 작업 응답과 일치하면서도, 조각 하나라도 누락·실패·시간 초과·취소·건너뜀·중복·비일치가 있으면 최종 판정과 원격 증거 검사(`remote-check`) 모두 실패로 닫힌다 | GitHub Actions `needs` 상태만 믿거나, 건너뛴 플랫폼 작업 항목을 무조건 거부해 단일 플랫폼·0변이 푸시를 막기 | H-63 완료 기준, 1회차 비판 추가 지적 1, 2회차 리뷰 지적 8 |
| Linux 계획·조각 실행의 선행 `test-diff`는 `test_diff::check_for_linux`를 사용한다: 전체 저장소 `.rs` 파일의 AST 변경 비교는 macOS와 동일하게 전부 수행하고, `cargo nextest list`만 `exclusions.json`의 macOS 전용 크레이트를 제외하며, `approval::apply_for_base`를 그대로 호출한다 | Linux에서 빌드할 수 없는 macOS 전용 크레이트 때문에 `nextest list`가 깨지지 않으면서도, macOS 전용 크레이트의 무단 테스트 수정은 AST 비교로 잡히고 H-62 승인 묶음·H-55 승인 행은 Linux와 macOS에서 동일하게 통과한다 | Linux에서 `test-diff`를 건너뛰기(무단 테스트 약화 뒤 변이 실행 위험) | H-62·H-63 완료 기준 |
| 분할·요약·합산·정책 검증 로직을 `xtask`와 `harness/ci/diff-mutants-{plan,shard,aggregate}.mjs`에 두고 워크플로는 호출만 한다 | 워크플로 YAML/JSON에 로직을 넣지 않아 로컬 단위·부정 변형 테스트로 전 경로를 검증할 수 있다 | 워크플로 인라인 셸 스크립트로 합산 구현 | H-63 완료 기준 |

## 추가 결정: 작업 브랜치 푸시에서 전체 CI 제외 (`master`·`merge/**` 한정)

- **배경**: 작업 브랜치(`d-58`, `h-64`, `s-24` 등)를 푸시할 때마다 `ci.yml`의 전체 검사(Linux·macOS·변경분 변이)가 실행되어 GitHub Actions 실행기(특히 macOS 슬롯)를 차지하고, 실제 병합 판정에 쓰이는 병합 커밋(`merge/*`)의 검사가 30분 넘게 대기열에 밀리는 병목이 발생했다. 작업 브랜치 푸시의 `ci.yml` 결과는 병합 판정에 쓰이지 않으며, 판정은 `merge/*` 병합 커밋과 `master`에서만 수행한다.
- **결정**:
  1. `.github/workflows/ci.yml`의 `on.push.branches`를 `["**"]`에서 `["master", "merge/**"]`로 제한한다. 일반 작업 브랜치 푸시에서는 로컬 범위 검사(`just check-fast`)와 원격 서명 검사(`signatures.yml`, `on.push.branches: ["**"]` 유지)만 돌고, 전체 CI(`linux`, `macos` / `macos-branch`, `diff-mutants-plan`, `diff-mutants-linux`, `diff-mutants-macos`, `diff-mutants-verdict`)는 `master`와 `merge/**` 푸시에서만 실행한다.
  2. `harness/ci/workflow-policy/policy.json`에 `ciPushBranches: ["master", "merge/**"]`와 `signaturesPushBranches: ["**"]`를 추가하고, `harness/ci/workflow-policy/check.mjs`의 `pushBranchFindings`로 `ci.yml`의 분기 확대(`**`, 추가 브랜치, `branches` 누락)·축소(`master` 또는 `merge/**` 누락, `merge/*` 오기)와 `signatures.yml`의 축소 모두 `event:push.branches`로 닫힌 실패하게 고정한다.
  3. H-59 규칙(`ci.yml` 최상위 `concurrency`: `master`는 실행·재시도별 독립 그룹 및 취소 금지, 그 외 브랜치는 브랜치별 이전 실행 취소)은 `merge/**` 브랜치에서 그대로 유지한다(`h59-capacity.test.mjs`).
  4. `remote-check.mjs`와 `merge-gate.mjs`는 `merge/*` 푸시의 `linux`, `macos-branch`, `diff-mutants-plan`, 실행된 조각, 건너뛴 플랫폼 항목, `diff-mutants-verdict`와 `signatures.yml`을 모두 요구하므로, `ci.yml` 실행이 없는 일반 작업 브랜치 SHA로는 병합 게이트가 닫힌 실패로 거부된다.
  5. `docs/harness.md`와 `docs/workflow.md`에 "작업 브랜치는 로컬 범위 검사 + 서명 검사만, 전체 검사는 `merge/**`·`master` 병합 커밋에서" 원칙과 H-63 병렬 변이 파이프라인을 반영한다.
- **버린 대안**: 모든 브랜치 푸시에서 `ci.yml`을 계속 돌리며 macOS 동시 실행 한도만 늘리기(계정 한도 제약으로 근본 해결 불가), 또는 `signatures.yml`까지 `merge/**`로 줄이기(작업 브랜치 푸시의 보호 경로 사후 탐지 약화).

## 기존 테스트 파일 기계적 갱신 내역 (AGENTS.md 5)

새 CI 작업 구조(`diff-mutants-plan`·`diff-mutants-linux`·`diff-mutants-macos`·`diff-mutants-verdict`), 공통 `macos-mutants-slot-{0,1}` 그룹, 그리고 `ci.yml` 푸시 트리거(`master`·`merge/**`)를 반영하기 위해 아래 7개 기존 테스트 파일을 기계적으로 갱신했다(기존 단언 약화·삭제 없음; 최종 `master` 반영은 사용자 서명 커밋 또는 잠금 시점 이전 메인 병합 절차로 진행한다).

| 파일 | 변경 요약 |
|---|---|
| `harness/ci/tests/workflow-policy.test.mjs` | `jobPermissions` 기대에 `.github/workflows/ci.yml#diff-mutants-verdict` 추가 |
| `harness/ci/tests/h52-ci-contract.test.mjs` | `ci.yml` 푸시 브랜치 `["master", "merge/**"]`·`signatures.yml` `["**"]`, `macos-branch` 120분 유지 및 `diff-mutants-{plan,linux,macos,verdict}` 계약 단언으로 갱신 |
| `harness/ci/tests/h52-remote-check.test.mjs` | `ci.yml` 필수 작업·단계 목록과 변이 조각 타임아웃 분류 픽스처를 새 작업 구조에 맞춤 |
| `harness/ci/tests/h52-review2.test.mjs` | `ci.yml` 필수 작업 픽스처에 `diff-mutants-plan`·`diff-mutants-verdict` 추가 |
| `harness/ci/tests/h52-review3.test.mjs` | 필수 단계 누락 테스트 픽스처에 `diff-mutants-plan`·`diff-mutants-verdict` 추가 |
| `harness/ci/tests/h59-capacity.test.mjs` | `mutationSlotConcurrency.group` 기대 문자열을 공통 `macos-mutants-slot-{0,1}`로 갱신 |
| `harness/ci/tests/ci-plumbing.test.mjs` | `ci.on.push.branches` 기대를 `["master", "merge/**"]`로, `signatures.on.push.branches` 기대를 `["**"]`로 분리 명시 |

## 보류 결정과 선택지

1. **조각 크기와 Linux 병렬도·macOS 선행 빌드 비용 튜닝**:
   - 현재 `MUTANTS_PER_SHARD = 20`, `MAX_SHARDS = 8`, Linux `max-parallel = 4`, 단계 제한 시간 Linux 90분 / macOS 150분이다. macOS 조각은 선행 `test-diff`에서 작업 공간 전체 `cargo nextest list` 콜드 빌드를 포함하므로 원격 CI 실행 시간과 공통 2슬롯 점유를 관측한 뒤 조정할 수 있다.
2. **실패 조각 재시도 운영 방식**:
   - `diff-mutants-aggregate.mjs`와 `remote-check.mjs`는 기존 H-52 계약과 동일하게 모든 작업의 `run_attempt` 일치를 요구한다. 조각 타임아웃·미배정 재시도 시 부분 재실행(Re-run failed jobs)이 아닌 **전체 재실행(Re-run all jobs)**을 사용한다.
3. **구 `harness/ci/diff-mutants.mjs`·`diff-mutants-verdict.mjs` 정리 시점**:
   - 현재 기존 H-54 테스트(`h54-diff-mutants.test.mjs`)를 깨지 않도록 두 스크립트를 보존했다. H-63 병합 뒤 원격 안정화가 확인되면 후속 정리 작업에서 폐기 여부를 결정한다.

## 결과

- `domain`·`application`·`order-execution` 대규모 변경(100~160건 이상)이 Linux ARM 병렬 러너에서 macOS 예산 소모 없이 완주한다.
- `store-sqlite` 변경도 CI와 야간 변이가 공유하는 `macos-mutants-slot-{0,1}` 2슬롯 상한 안에서 최대 8조각으로 병렬 실행되고, `diff-mutants-verdict`가 모든 조각의 완료·성공과 건너뛴 플랫폼 항목을 닫힌 실패로 합산한다.
- 바뀌면 다시 볼 조건: Linux ARM 러너 한도나 `store-sqlite` 변이 규모가 커져 `MUTANTS_PER_SHARD = 20` 또는 `MAX_SHARDS = 8`로 90/150분 안에 끝나지 않을 때, 또는 계정 macOS 러너 한도가 바뀔 때.

## 손 변형 검증 (격리 임시 복제본, 13/13 검출)

| 번호 | 파일 | 변형 | 잡은 테스트 | 결과 |
|---|---|---|---|---|
| M-13 | `crates/xtask/src/mutants_check/scope.rs` | `TargetPlatform::Linux` 필터 반전 | `h63_platform_filter_routes_neutral_crates_to_linux_and_excluded_to_macos` | 검출(실패) |
| M-14 | `crates/xtask/src/mutants_check.rs` | 조각 실행에서 `--platform all` 허용 | `h63_platform_cli_accepts_canonical_plan_and_shards_and_rejects_invalid` | 검출(실패) |
| M-15 | `crates/xtask/src/test_diff.rs` | `check_for_linux`에서 AST `compare_sources` 생략 | `h63_linux_precondition_accepts_h62_bundle_and_rejects_unapproved_change` | 검출(실패) |
| M-16 | `harness/ci/diff-mutants-plan.mjs` | 조각 수 상한 `MAX_SHARDS = 8` 제거 | `harness/ci/tests/h63-diff-mutants.test.mjs` | 검출(실패) |
| M-17 | `harness/ci/diff-mutants-plan.mjs` | 미선언 파일 경로를 예외 대신 `domain`으로 기본 라우팅 | `harness/ci/tests/h63-diff-mutants.test.mjs` | 검출(실패) |
| M-18 | `harness/ci/diff-mutants-shard.mjs` | 조각 요약에서 `timeout.txt` 비어 있음 검사 생략 | `harness/ci/tests/h63-diff-mutants.test.mjs` | 검출(실패) |
| M-19 | `harness/ci/diff-mutants-shard.mjs` | `MUTATION_OUTCOME` 누락 시 통과 허용 | `harness/ci/tests/h63-diff-mutants.test.mjs` | 검출(실패) |
| M-20 | `harness/ci/diff-mutants-aggregate.mjs` | 누락된 조각 작업 무시 | `harness/ci/tests/h63-diff-mutants.test.mjs` | 검출(실패) |
| M-21 | `harness/ci/diff-mutants-aggregate.mjs` | 페이지네이션 잘림(`total_count` 불일치) 검사 생략 | `harness/ci/tests/h63-diff-mutants.test.mjs` | 검출(실패) |
| M-22 | `harness/ci/workflow-policy/check.mjs` | `diff-mutants-macos` 동시 실행 상한 2 → 4 완화 | `harness/ci/tests/h63-diff-mutants.test.mjs` | 검출(실패) |
| M-23 | `harness/ci/remote-check.mjs` | `checkRun`에서 실패한 조각 작업 거부 생략 | `harness/ci/tests/h63-diff-mutants.test.mjs` | 검출(실패) |
| M-25 | `harness/ci/diff-mutants-aggregate.mjs` | 건너뛴 플랫폼 작업에 실행 단계가 있어도 허용 | `harness/ci/tests/h63-diff-mutants.test.mjs` | 검출(실패) |
| M-26 | `harness/ci/workflow-policy/check.mjs` | CI·야간 macOS 변이 공통 2슬롯 대기열 공유 검사 생략 | `harness/ci/tests/h63-diff-mutants.test.mjs` | 검출(실패) |
