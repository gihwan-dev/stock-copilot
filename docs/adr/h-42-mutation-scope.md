# H-42: 변경 전·후 참조 폐쇄와 독립 밤 변이 검사

- 상태: ADR 초안, 메인 평가 대기
- 원래 기준: af70b46fc6c62a0d1a7b036c9977b3d2f3d47366
- 적대적 리뷰 대상: 42f09e74fe9fa3cec35ba7e2d9c79408a8389491
- ADR 0013의 crate 범위 및 check-full 중복 전체 실행 결정을 대체한다. 생존 허용·시간 초과 실패·대상 글롭·기존 테스트 보호는 유지한다.

## 배경

처음의 파일 단위 및 내향 직접 참조만으로는 다른 파일을 검사하는 테스트의 약화를 놓쳤다. portfolio의 1단위 경계 테스트가 exact_arithmetic의 변이를 잡는 경로, holding에서 exact 호출을 제거한 경우의 과거 간선을 함께 보존해야 한다. 단순한 줄 변경 필터는 사용하지 않는다.

| 변경/판정 | 범위 |
|---|---|
| 대상 crate 안 정상 Rust 파일(대상 글롭 밖 포함) | 기준·현재 그래프의 외향 전이 폐쇄 + 내향 한 단계, 마지막에 대상 글롭과 교차 |
| 같은 파일 테스트만 변경 | 구현 변경과 같은 규칙 |
| tests/·테스트 지원·crate Cargo.toml·비 Rust 지원 파일·lib 비재수출 변경 | crate의 대상 전체 |
| 삭제·이름 변경·읽기/파싱/모듈 해석 실패 | crate의 대상 전체 |
| 루트 Cargo.toml/Cargo.lock/툴체인/mutants/nextest 설정 | 모든 대상 |
| 대상 crate 밖 일반 파일 | 없음. 다른 crate를 통한 영향은 밤 전체 검사 책임 |

## 병렬 실행

기본 jobs는 min(max(floor(available_parallelism / 2), 1), 6)이다. STOCK_COPILOT_MUTANTS_JOBS로 낮출 수 있고 ASCII 숫자로 된 1부터 기본값까지의 값만 허용한다. 부호·공백·유니코드 숫자·0·오버플로·상한 초과를 거부한다. timeout-multiplier는 5*jobs, 공유 jobserver와 baseline을 유지하고 시간 초과는 실패다. [공식 병렬 지침](https://mutants.rs/parallelism.html)의 CPU·메모리·디스크 경합 경고와 [시간 제한 지침](https://mutants.rs/timeouts.html)을 근거로 한다. nextest wrapper와 sandbox 프로필은 수정하지 않았다. 새 의존성은 없다.

같은 money.rs의 33개 변이 직접 실행, 각 한 번, baseline 포함. 앞선 H-42 측정이며 다른 작업 사본들의 빌드·테스트 부하가 있던 환경이다.

| jobs | 전체 초 | caught | unviable | missed/timeout |
|---|---:|---:|---:|---:|
| 2 | 189.21 | 20 | 13 | 0/0 |
| 4 | 160.02 | 20 | 13 | 0/0 |
| 6 | 127.66 | 20 | 13 | 0/0 |

환경 허용 형식은 변수 이름까지 지정할 수 없다. 전용 environment.rs의 clock-env:var 허용은 유지하되 AST 테스트가 var 경로 하나, import 없음, 리터럴 변수 이름 STOCK_COPILOT_MUTANTS_JOBS 하나를 고정한다.

## 범위와 참조 그래프

변경 전 git 객체와 작업 사본의 모든 crate 내부 `.rs`를 각각 읽어 그래프를 만든 뒤 간선을 합친다. 대상 밖 `.rs`도 노드와 시작점이다. 외향 간선은 전이 폐쇄하고, 변경 파일을 참조하는 내향 간선은 한 단계만 추가한 뒤 마지막에 대상 글롭과 교차한다. 참조 삭제도 기준 판의 간선으로 남는다. 이름 인덱스는 모듈 이름·정의된 타입/함수/상수·impl 소유 타입을 파일 집합으로 대응시키고 재수출 및 use 별칭을 고정점까지 전파한다. 구현과 인라인 테스트의 토큰을 함께 읽는다. 같은 이름이나 지역 가리기는 모든 후보 소유 파일을 포함하는 과대 근사이며, 정밀한 Rust 이름 해석기로 주장하지 않는다. `super`는 부모 디렉터리의 후보를 모두 더한다.

모호한 glob import/reexport, `#[path]`, 속성 있는 외부 모듈, `foo/mod.rs`, 복잡한 impl 대상은 crate 전체로 넓힌다. crate 내부 파일에 매크로 호출이 있으면 매크로 펼침의 의미를 해석하지 않고 전체로 넓힌다. `assert_eq!`·`proptest!`도 이 규칙에 포함한다. 그 결과 현재 domain 대표 다섯 범위는 모두 51/51파일·1,200/1,200변이(100%)다. 이는 의도적인 안전 우선 결과이며 이번 수정에서 파일 범위 절감을 얻었다고 보고하지 않는다. 병렬 실행과 푸시 중복 제거 효과는 유지한다. 실제 domain 소스와 다섯 변경 경로를 그래프에 넣어 선택 파일 및 cargo-mutants --list 결과를 열거했고 실행한 변이 수로 혼동하지 않는다. 기록은 target/h42/representative.tsv다.

| 대표 변경 | 선택 파일 | 파일 비율 | 변이 | 변이 비율 |
|---|---:|---:|---:|---:|
| account_id.rs 잎 모듈 | 51/51 | 100% | 1200/1200 | 100% |
| money.rs | 51/51 | 100% | 1200/1200 | 100% |
| exact_arithmetic.rs | 51/51 | 100% | 1200/1200 | 100% |
| portfolio_valuation.rs | 51/51 | 100% | 1200/1200 | 100% |
| money.rs 인라인 테스트만 | 51/51 | 100% | 1200/1200 | 100% |

회귀 (a)는 portfolio 테스트의 경계 호출 네 줄을 기준 판에 둔 뒤 지우고도 exact_arithmetic이 선택되는 것을 확인한다: weakened_portfolio_boundary_selects_exact_arithmetic. (b)는 holding의 exact 호출을 같은 역할의 지역 함수로 바꾸고 기준 판에서 exact를 계속 선택한다: removed_holding_reference_keeps_baseline_exact_arithmetic. 둘 다 fallback 전체 글롭을 허용하는 단언이 아니라 exact 파일 경로가 실제 그래프 결과에 있어야 통과한다.

## 밤 검사와 운영 규칙

nightly의 mutants 작업은 changes/linux/macos에 needs가 없으며 다른 검사 작업의 실패로 건너뛰지 않는다. 기존 Linux 검사에서 변이 단계를 옮겼다. `--shard`는 로컬 27.1.0 도움말에서 확인했고 0/4~3/4의 고정 네 분할만 xtask가 받는다. matrix fail-fast=false로 한 shard가 실패해도 나머지를 취소하지 않는다. 각 shard는 보호된 전체 대상 글롭을 그대로 사용하고 네 결과의 합집합이 전체 변이다. 작업 제한은 각 120분이다. notify-mutants는 이 작업만 needs로 두고 failure()에서 제목 '밤 변이 검사 실패'의 기존 알림 스크립트를 실행한다. 전체 workflow 기대값과 정책 사례도 함께 바꾸고, strategy는 이 특정 작업의 고정 네 분할만 허용한다. 다른 작업에 strategy를 여는 정책 변경은 하지 않는다. 사용하지 않게 된 기존 단일 전체 변이 명령은 workflow 명령 허용 목록에서 제거했다.

[GitHub 공식 러너 사양](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)에 따르면 공개 저장소 ubuntu-24.04-arm은 4 vCPU·16GB다. jobs 공식으로 2다. 이전 동일 범위 jobs 2는 33개에 189.21초(총시간/변이=5.74초), 준비·baseline 포함이었다. 1,200~2,000개면 단일 작업 약 115~192분으로 기존 180분 한도를 넘을 수 있다. 네 shard면 300~500개로 약 29~48분, 설치·baseline 20분과 러너 차이/경합에 대한 변이 시간 2배 여유를 더하면 약 78~116분이다. 각 shard 120분을 사용한다. 이는 macOS 실측을 Linux에 옮긴 예산 추정이며 원격 실행 보장이 아니다. 아래 수정 회차의 jobs 2 재실측도 별도로 기록한다.

**밤 변이 실패 이슈가 열려 있으면 메인은 다음 병합 전에 해결한다.** 로그의 생존/시간 초과/인프라 실패를 분류하고 원인을 고친 뒤 네 shard 전체 성공을 확인한 후 이슈를 닫는다. 푸시 훅은 오프라인을 유지하며 GitHub에 결과를 조회하지 않는다. 따라서 기계적인 다음 푸시 차단이 생겼다고 주장하지 않는다.

## 남는 구멍

이번에 닫은 것: portfolio 테스트가 root 재수출 Money/compare_money를 통해 exact_arithmetic을 검사하는 간접 경로, holding이 exact 호출을 지운 변경의 과거 간선, 대상 밖 application 파일에서 주문 게이트로 연결되는 경로, use 별칭·super의 내부 후보, mod.rs와 매크로의 불확실성을 좁게 통과시키는 경로다.

여전히 남는 것: 다른 crate의 구현 변경이 domain 테스트의 생존 여부에 영향을 주는 간선은 crate 내부 그래프에 없다(예: testkit의 비교 도우미 수정). 이는 밤 전체 변이로 탐지를 늦춘다. 매크로가 만드는 참조는 정밀하게 해석하지 않으며 현재는 감지한 매크로가 참조 자체를 숨길 수 있으므로 그 crate 전체로 넓힌다. derive 및 그 밖의 해석하지 못한 속성도 매크로 불확실성으로 다룬다. proc macro가 의존성 내부에서 생성한 코드 자체의 변이는 cargo-mutants가 보장하지 않는다. cargo-mutants가 생성하지 않는 변이, 보호된 대상 밖 코드, 외부 빌드 도구의 의미는 밤 검사로도 증명하지 못한다. 이름 가리기와 동명 정의는 현재 후보 합집합으로 과선택하므로 범위가 커질 수 있다.

수정 회차 재실측: 같은 money.rs 33개를 jobs 2로 실행한 총시간 81.56초(2.47초/변이), caught 20·unviable 13·missed/timeout 0, 종료 0. 설치/러너 차이와 부하 변화를 숨기지 않기 위해 CI 예산은 이 값보다 느린 앞선 5.74초/변이를 사용했다. 로그는 target/h42/revision-jobs2.log와 .json이다. scope/graph 손 변형 9개(대상 밖 시작점 제외, 지원 파일 확장 제거, lib 확장 제거, ASCII 검사 제거, mod.rs 확장 제거, 매크로 확장 제거, 내향 간선 제거, 외향 폐쇄 제거, 기준 그래프 합치기 제거)는 모두 컴파일 후 테스트 실패(종료 100)로 검출했다. 원복 후 관련 테스트가 통과했다.

메인 문서 변경 문장: harness.md 6.4에 '변경 전·후 crate 내부 의존 그래프의 외향 전이 폐쇄와 내향 직접 참조를 선택하며, 해석 불가는 전체로 넓힌다. 밤 전체 변이는 독립된 네 shard로 실행한다. 밤 변이 실패 이슈가 열려 있으면 다음 병합 전에 해결한다'를 반영한다. docs/agents/implementer.md에는 '밤 변이 실패 이슈가 열려 있으면 구현자는 관련 실패와 수정 검증을 보고하고, 메인은 네 shard 성공을 확인하기 전 다음 병합을 진행하지 않는다'를 더한다. docs/workflow.md 2.1의 메인 병합 전 확인 의무에도 같은 문장을 적는다. 푸시 훅의 온라인 조회는 추가하지 않는다.

cargo-mutants의 --help 예시는 1/4지만 실제 열거 실행에서 4/4를 거부하여 0기반 분할임을 확인했다. 최종 설정과 파서는 0/4, 1/4, 2/4, 3/4이며 4/4는 거부한다. 도움말에 의존한 추측을 실제 CLI 열거로 보완했다.

최종 분할 열거 검증은 전체 1,200개와 각 shard 300개를 얻었고, 네 집합의 합집합이 전체와 같으며 서로 겹치지 않음을 확인했다(target/h42/revision-shards.json).

## 최종 검증과 승인 예외

최종 전체 check 로그는 target/h42/revision-final-check.log다. Rust workspace 522/522, 커버리지 실행 256/256 및 coverage-check, 형식/Clippy/TS/구조/생성물 검사가 통과했다. 끝의 test-diff는 원래 기준 af70b46 대비 이미 승인된 3건(기존 check-full 테스트 removed/not-listed, 환경 경계 기대 집합 테스트 modified)만 보고했다. 메인이 이 3건을 승인하고 LEFTHOOK=0 커밋을 명시적으로 지시했다. 검사 기준을 바꾸거나 test-diff를 수정하지 않는다. 새 검사 끄기 0건, hooks-installed 통과다.

최신 그래프에서 손 변형 9개를 다시 실행해 모두 테스트 실패(종료 100)로 검출했고 원복 후 변이 하네스 관련 24개 테스트와 Clippy가 통과했다. CI 정책/작업 테스트는 67개 통과했다. 실제 GitHub 밤 실행과 알림 이슈 생성은 실행하지 않았다.

이번 수정에서 기존 master Rust 테스트를 추가로 변경하지 않았다. 이 브랜치의 scope_tests를 수정·확장했고, 기존 CI JavaScript 테스트 두 파일(ci-plumbing.test.mjs, workflow-policy.test.mjs)은 이동한 변이 단계와 새 알림 권한 기대값만 바꿨다. 보호 변경은 nightly.yml, justfile와 그 check-list 기대값, workflow-policy 검사/정책/사례/예제, 변이 xtask 구현과 자체 테스트, 이 ADR이다. 생성 설정·잠금 조각·훅 해시는 쓰기 모드로 재생성했다.
