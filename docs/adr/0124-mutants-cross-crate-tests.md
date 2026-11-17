# ADR 0124: H-49 변경분 변이 검사가 다른 crate의 통합 테스트를 고름

- 상태: 확정 (2026-11-17)


## 결정

변이 소스 선택(파일과 diff 교집합)은 유지하고 실행 테스트 패키지를 별도로 명시한다. ADR 0091이 선언한 application의 포트 가짜 기반 통합 테스트 소유자는 testkit-ports다. A-12(ADR 0119)의 두 변이와 A-25(ADR 0120)의 네 변이가 이 연결 없이는 검출되지 않았다.

`mutants_check/test_packages.rs`의 좁은 명시 목록을 저장소의 선언으로 삼는다. application에는 application과 testkit-ports, domain에는 domain, order-execution에는 order-execution, store-sqlite에는 store-sqlite를 고른다. application이 포함된 여러 대상이면 중복 없는 합집합을 cargo-mutants 27.1.0의 `--test-package`로 전달한다. cargo-mutants 27.1.0은 정상 소스 baseline에서 --test-package를 반영하지 않는다. 같은 선택 집합을 --cargo-arg=--package=<패키지>로도 전달하여 baseline의 빌드·테스트와 변이 실행을 일치시킨다. 외부 테스트가 원래 깨졌을 때 검출로 잘못 세지 않는 회귀를 둔다. 이름 추측이나 전체 역의존 폐쇄를 쓰지 않는다. 알 수 없는 소스 소유자는 실패한다. 새 변이 대상이나 외부 테스트 소유자를 추가할 때 이 선언과 회귀를 함께 갱신한다.

변경분·일일·주간 전체는 기존 공통 run_with_diff를 사용하므로 같은 규칙을 적용한다. 주간도 기존 기본값(변이 crate 테스트만 실행)에 같은 공백이 있었다. catalog·shard 분할은 소스 기준으로 유지한다. workspace 전체 테스트로 확대하지 않는다. application이 없는 실행에는 테스트 패키지 인자를 추가하지 않아 기존 cargo-mutants의 패키지별 테스트 선택과 명령을 그대로 유지한다. domain 단독도 기존과 동일하다. 여러 소스 패키지를 한 실행으로 검사할 때 합집합 테스트를 모든 변이에 실행하는 비용은 감수한다.

외부 패키지 추가, 제품 소스 수정, 제외 목록 확장, 검사 끄기 없음. 프로젝트 신뢰·훅 정의 신뢰·현재 세션 적용은 확인 불가이며 지시문의 계속 진행 예외를 적용한다. frozen install 및 lefthook install 성공. 원본 지시문은 수정하지 않았다.

## 검증

이하 절에 실제 실행 결과와 시간을 기록한다.

## 인계

| 받는 작업 | 내용 |
|---|---|
| H-47 | 원격 일일·주간 실행의 완주와 확대된 application 테스트 시간 확인. 로컬 실행을 원격 완주 증거로 해석하지 않음 |

## 승인된 기존 테스트 변경

`crates/xtask/src/mutants_check/tests.rs`의 `nightly_allows_an_empty_shard_only_after_a_nonempty_catalog` 준비를 변경했다. 루트 h47-empty 패키지/src 글롭에서 가상 workspace의 crates/domain 패키지/domain 이름/소스 글롭으로 옮겼다. 알 수 없는 소유자는 실패하는 새 계약에 맞춘 것이다. 기존 단언은 모두 보존했다. 공유 prepared_workspace 도우미도 실제 domain만 들어 있는 예제의 examine_globs를 domain만으로 고정했다. weak_test_example_leaves_surviving_mutants의 기존 단언은 유지한다. 준비 변경은 총 두 곳, test-diff는 support-modified(prepared_workspace) 1건과 modified(nightly_allows_an_empty_shard_only_after_a_nonempty_catalog) 1건, 합계2건이다. 외부 의존성 추가는 없다.

보호 파일 변경은 실행기와 회귀, docs/harness.md의 계약 설명, 새 ADR 등록에 따른 .codex/config.toml 및 harness/claude-hooks/hashes.json 재생성이다. 보호 생성물은 저장소 생성기로만 갱신했다.

## 시간 측정 1차

별도 최소 workspace, 같은 5개 변이, cargo-mutants 27.1.0/nextest/jobs2. 다음 값은 로컬 실제 벽시계이며 동시 작업·캐시 편차가 포함된다. 제품 전체나 원격 CI 실행 시간으로 일반화하지 않는다.

| 대상 | 전(초) | 후(초) | 전 검출/미검출 | 후 검출/미검출 |
|---|---|---|---|---|
| application | 5.872 | 7.115 | 0/5 | 5/0 |
| domain | 6.278 | 10.708 | 5/0 | 5/0 |

domain의 테스트 선택은 전후 동일하지만 이 표본의 벽시계는 증가했다. 실행 환경의 경합을 분리한 반복 측정으로 확인한다. 성능 무회귀가 이 단일 표본에서 입증됐다고 해석하지 않는다.

## 보완 실행

A-12·A-25 실제 소스 submit_estimate.rs와 run_full_inspection.rs를 두 테스트 패키지로 검사한 결과 87초, 변이8·검출5·컴파일불가3·미검출0·시간초과0이었다. 이 두 파일에서는 신규 미검출이 없었다. application 전체 소스의 완전 검사는 실행하지 않았다. 공식 mutants-diff는 당시 기존 fixture 변경1건의 test-diff 사전 검사로 종료1이며 변이 실행은 NOT_RUN이다.

## domain 반복 실측

최종 구현에서 domain은 CLI 인자까지 기존과 동일하다(별도 회귀 단언). 전·후를 교대로 세 번씩 실제 실행했다. 각 실행의 변이는5개·검출5·미검출0·종료0으로 같다. 전 5.397/4.357/6.371초, 후 4.512/4.733/6.008초, 중앙값은 5.397→4.733초다. 측정 분산을 포함한 로컬 소표본이며 속도 개선 자체를 주장하지 않는다. 원래 domain 테스트 범위와 인자가 유지됨을 확인한 결과다.

## 최종 baseline 일치 구현 시간

최종 인자(--test-package와 --cargo-arg의 같은 선택 집합)로 최소 예제를 다시 실행했다. application 3.464→4.050초(미검출5→검출5), domain 3.700→3.567초(양쪽 검출5). domain의 전후 명령은 동일하며 application의 추가 시간은 외부 테스트를 사전 실행부터 포함한 비용이다. 실행은 로컬 캐시·동시 부하를 포함한다.

첫 전체 실행은 1993개 중1897개를 실행하여1896통과·기존 최소 workspace fixture 한 건 실패,96미실행으로 끝났다. 사전 실행이 존재하지 않는 패키지를 거부한 결과이며 fixture 선언을 실제 domain 구성에 맞게 고쳤다. 전체 Rust 테스트와 커버리지는 수정 후 다시 검증한다.

## 구현 커밋 전 검사

check-fast는 docs·xtask 변경 때문에 전체 검사로 승격됐다. JavaScript·설정·훅·CI128분할 열거 회귀, 전체 clippy, deny 네 검사, arch-check, source-check, SQL·생성물·raw-types·disable-check를 통과한 뒤 최소 fixture 한 건에서 실패했다. 해당 준비를 수정한 뒤 전체 Rust1993/1993 통과(속성4096), 커버리지1114/1114 통과·coverage-check 위반 없음, xtask clippy 통과를 확인했다. 이후 disable-report·disable-report-since·hooks-installed도 통과했다. 새 끄기는0건이다.

test-diff의 최종 실패는 승인된 준비 변경2건뿐이다. 사용자 지시문의 (d)에 따라 이 구현 커밋에만 LEFTHOOK=0을 적용한다. 검사 규칙·단언을 약화하지 않는다. 최종 문서 커밋과 최신 master 통합 후에는 정상 훅의 전체 검사를 다시 확인한다.

## 1차 리뷰 평가와 수정

새 Opus의 중요 두 건을 새 Codex가 비판했다. 혼합·주간 합집합의 비용 증가는 최소 예제로 확인했지만128분할 CI의160분 위반은 실측되지 않았다. domain 변이를 외부 테스트가 잡는 것은 실제 실패 단언에 의한 정상 검출이며 명시한 단독/혼합 계약에 부합한다. 시간은 같은 domain변이5개에 단독3.705초·혼합5.342초·주간4.317초였다. 제품 전체나 CI 예산으로 환산하지 않는다. H-47이 실제 조각별 최대 시간과 빌드·테스트 비용을 확인한다.

실행으로 확정된 연결부 회귀 공백은 application이 설정된 상태에서 domain파일만 선택하는 회귀로 보강한다. domain단독은5개 생존, 혼합은 외부 테스트에 의해 검출되어야 한다. 외부 테스트 변경의 주간 안내도 같은 COMPANION 선언의 명시 소스/패키지/경로 관계로 보강한다. 소스 변이 선택은 확대하지 않는다. broker-toss의 import는 소유 선언의 증거가 아니며 SecretValue 손 변형은 application자체 테스트3개로 검출되어 범위를 추가하지 않는다. 번호 없는 초안 형식은 사용자 지시를 유지하고 자리표시자만 정리한다.

수동 commit-guard에 기준SHA를 인덱스 경로로 잘못 전달한 호출은 무효 진단이었다. 즉시 올바른 무인자 just commit-guard로 재검사하여 위반 없음을 확인했다. 이 호출 실수와 승인된 test-diff 예외를 구별한다.

1차 수정의 정상 커밋 훅은 Rust1996/1996·커버리지1114/1114와 기준 검사까지 통과하고, master분기점 대비 최초 승인 준비2건의 test-diff에서만 종료1이었다. 준비 부분을 추가 변경하지 않았다. 정상 훅 성공으로 기록하지 않으며 같은 승인2건 범위의 커밋 한정 예외를 적용한다. 후속 끄기 보고·훅 설치 확인을 유지한다. 전체 target_globs 연결 손 변형은 컴파일 성공 후 새 domain회귀가 실패(생존0 대 기대5)하여 검출했다.

## 2차 비판 후 회귀 보강

application없는 두 패키지 조합3가지와 세 패키지 조합을 새 테스트 하나로 고정했다. 제품·실행기 구현은3cf4a7aa와 바이트 동일하며 기존 단언을 바꾸지 않았다. 모듈4개 회귀 통과와 xtask clippy로 검증한다. packages.len()>1 손 변형은 새 회귀에서 실제 검출된다. 최초 승인 준비2건은 master분기점 검사에 계속 나타나므로 동일 범위의 커밋 예외로 기록한다. 최신 master수신 뒤 최종 HEAD 전체 검사를 별도로 수행한다.

## 3차 최종 비판 후 보강

원본의 실제 결함은 없으나 비-domain단독·빈입력 인자 보존의 테스트 공백이 실행으로 확정됐다. 기존 단언을 유지하고 application없는8개 부분집합과 포함8개, 총16개 조합의 정확한 인자 목록을 추가한다. Command에 이미 있는 mutants·cap-lints 인자도 보존해야 한다. 새 모듈6개 회귀·xtask clippy 통과. 실행기 구현은253a01c7과 바이트 동일하다. 비판자가 확인한 등가 변형은 검출 수로 요구하지 않는다. 최대3회차 검토를 마쳤고4차 위임은 하지 않는다.

최신 master c22117c94038c27d252fdf220d88919605afde54는 병합00dc11fd2bd64e9385196858c02e990b5d693f50로 수신했다. 생성 설정·해시 충돌만 정식 생성기로 해결했다. 정상 훅의 Rust2001/2001·커버리지1114/1114와 기준 검사 통과 뒤 승인 준비2건 test-diff만 실패했다. 후속검사 뒤 같은두건한정 예외로 병합했으며 정상훅 성공으로 기록하지 않는다. tasks/progress 변경은 master A-24 수신분이다.

마지막 검토 후 수정 범위는253a01c77068c8515d53e6a591782104ac1d6d4c → 2fa0719e23009019fe9c1135f080753292088aba다. 최신 master 수신을 포함하며 H-49 실행기 바이트는 그대로다. 추가한 두 테스트가16개 조합의 인자 목록과 기존 인자 보존을 검증한다. 최종6개 모듈 회귀·clippy 통과 및 아래 손 변형을 실제 확인했다. 추가4차 독립 리뷰는 없다.


## 최종 실행 증거

최신 통합 제품 소스에서 A-12·A-25 두 파일의 별도 자동 변이를 다시 실행했다. application/testkit-ports와 같은 baseline 패키지 인자를 명시했고,8개 중 검출5·컴파일불가3·생존0·시간초과0, 실제 264.441초였다. 제품 application/testkit-ports 소스는 수신한 master c22117c9와 diff가 없다. application전체 변이 검사는 수행하지 않았다.

최종공식 just mutants-diff c22117c94038c27d252fdf220d88919605afde54는 최초 승인 준비2건의 test-diff에서 종료1이며 자동 변이 NOT_RUN이다. 이를0변이 성공이나생존0으로 표시하지 않는다. 하네스 변경 검출력은 위32개 컴파일가능 손 변형 및 실제연결 회귀로 별도 확인했다.


## 최신 master 추가 수신과 검사 재시도

최종 검증 중 master가7b74fc842659ae96b65a9c64e1de085f2c08385b(DS-19)로 전진하여 추가 수신한다. 충돌은 생성 설정·해시 두 파일뿐이며 저장소 생성기로 재생성했다. H-49 실행기·테스트 소스에는 master 측 변경이 없다. 앞선 c221수신 기록은 이전 통합 단계의 증거로 보존한다.

9c54ace6에서 첫 전체 검사는2003개 중1784개 실행·1783통과·기존 h44_real_merge_hook_accepts_incoming_approved_changes 1건 실패,219개 미실행이었다. 실패는 fixture 최초 work커밋의 성공 단언이며 하위 출력이 보존되지 않아 원인은 확정하지 못했다. 소스 변경 없이 같은 SHA·속성4096 조건으로 해당 회귀를 격리 실행해1/1 통과했다. Cargo 빌드 병렬도2로 재시작한 전체 검사는 새 master를 수신하기 위해 자신이 띄운 프로세스 그룹15871만 SIGINT로 종료했고 종료130을 성공으로 세지 않았다.

이후 통합·최종 전체 검사는 CARGO_BUILD_JOBS=2로 빌드 자원만 제한한다. nextest 기본 병렬 설정·선택·단언·속성4096·검사 항목은 유지한다. 원인을 메모리나 H-49 결함으로 단정하지 않으며 기존 회귀·규칙을 수정하지 않는다. 실제 최종 결과와 SHA는 최종 보고와 별도 실행 로그에 남긴다.


추가 master7b74 통합의 정상 훅은 CARGO_BUILD_JOBS=2에서 Rust2003/2003(414.488초)·커버리지1114/1114(211.257초) 및 기준 검사까지 통과했다. 기존H44 회귀도 전체 실행에서 통과했다. 최초 승인 준비2건의 test-diff만 종료1로 남았으며 정상훅 자체를 성공으로 표시하지 않는다. 동일승인2건 한정 예외로 병합을 확정한 뒤 최종커밋 HEAD 전체검사를별도실행한다. source·단언·속성4096·nextest기본parallel은변경하지않았다.

## 최종 선택·안내 모듈 손 변형

최종 모듈을 mktemp 임시 복사본에서 원본 테스트와 함께 rustc --test로 실제 컴파일·실행했다.33개 중 컴파일 성공32개·검출32개, 컴파일 불가1개 집계 제외. 실행기 연결부 변형은 임시 전체 복사본의 nextest 회귀로 검출했고 독립 비판자도 연결·안내 제거를 재검증했다.

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|
| 1 | `"crates/domain" => "crates/wrong"` | 검출 | test_packages::tests |
| 2 | `"crates/application" => "crates/wrong"` | 검출 | test_packages::tests |
| 3 | `"crates/order-execution" => "crates/wrong"` | 검출 | test_packages::tests |
| 4 | `"crates/adapters/store-sqlite" => "crates/wrong"` | 검출 | test_packages::tests |
| 5 | `"domain", None => "wrong", None` | 검출 | test_packages::tests |
| 6 | `"application", Some => "wrong", Some` | 검출 | test_packages::tests |
| 7 | `"order-execution", None => "wrong", None` | 검출 | test_packages::tests |
| 8 | `"store-sqlite", None => "wrong", None` | 검출 | test_packages::tests |
| 9 | `Some(COMPANION.1) => None` | 컴파일 불가 | 집계 제외 |
| 10 | `"testkit-ports" => "wrong"` | 검출 | test_packages::tests |
| 11 | `glob.starts_with => glob.ends_with` | 검출 | test_packages::tests |
| 12 | `glob.starts_with => glob.contains` | 검출 | test_packages::tests |
| 13 | `{directory}/src/ => {directory}/` | 검출 | test_packages::tests |
| 14 | `{directory}/src/ => {directory}/tests/` | 검출 | test_packages::tests |
| 15 | `for glob in globs { => for glob in globs.iter().skip(1) {` | 검출 | test_packages::tests |
| 16 | `for glob in globs { => for glob in globs.iter().take(1) {` | 검출 | test_packages::tests |
| 17 | `packages.insert(String::from(*package)); => packages.remove(*package);` | 검출 | test_packages::tests |
| 18 | `packages.insert(String::from(*name)); => packages.remove(*name);` | 검출 | test_packages::tests |
| 19 | `Ok(packages) =>     Ok(BTreeSet::new())` | 검출 | test_packages::tests |
| 20 | `Ok(packages) =>     packages.insert(String::from("testkit-ports"));     Ok(packages)` | 검출 | test_packages::tests |
| 21 | `packages.contains("testkit-ports") => !packages.contains("testkit-ports")` | 검출 | test_packages::tests |
| 22 | `packages.contains("testkit-ports") => packages.len() > 1` | 검출 | test_packages::tests |
| 23 | `packages.contains("testkit-ports") => packages.contains("testkit-ports") || (packages.len() == 1 && !packages.contains("domain"))` | 검출 | test_packages::tests |
| 24 | `let packages = selected(globs)?; => let packages = selected(globs)?; if globs.is_empty() { command.arg("--test-package=application"); }` | 검출 | test_packages::tests |
| 25 | `let packages = selected(globs)?; => *command = std::process::Command::new("cargo"); let packages = selected(globs)?;` | 검출 | test_packages::tests |
| 26 | `command.arg("--test-package").arg(package); => command.arg("--package").arg(package);` | 검출 | test_packages::tests |
| 27 | `command.arg("--test-package").arg(package); => command.arg("--test-package").arg("application");` | 검출 | test_packages::tests |
| 28 | `Ok(packages) =>     Err(String::from("failure"))` | 검출 | test_packages::tests |
| 29 | `glob.starts_with(&format!("{directory}/src/")) => !glob.starts_with(&format!("{directory}/src/"))` | 검출 | test_packages::tests |
| 30 | `&& changed => || changed` | 검출 | test_packages::tests |
| 31 | `path.starts_with => path.ends_with` | 검출 | test_packages::tests |
| 32 | `format!("{}/", COMPANION.2) => format!("{}", COMPANION.2)` | 검출 | test_packages::tests |
| 33 | `COMPANION.0)]) => COMPANION.1)])` | 검출 | test_packages::tests |
