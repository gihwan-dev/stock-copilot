# ADR 0013: Rust 테스트 하네스 (커버리지, `test-diff`, 변이 테스트, 테스트 네트워크 차단)

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-11(Rust 쪽), H-29(Rust 쪽), H-17, H-18
- 반영한 문서: 없음 (harness.md 1.1, 1.2, 3.5, 6.1~6.4, 7.7, R-09 Q6·Q7·Q9, R-15 2.7, ADR 0005·0006·0008·0011을 따르고 문서가 정하지 않은 곳만 여기 적는다)

## 맥락

harness.md 6.1은 파일별 줄 커버리지 기준을, 6.3은 보호된 예외 목록 밖의 실행되지 않은 줄을 실패로, 6.4는 `cargo-mutants`의 대상과 생존 허용 목록을, 7.7은 기준 커밋 대비 기존 테스트의 삭제·변경과 `nextest list` 개수 감소를, 6.2는 `sandbox-exec` 테스트 네트워크 차단을 정했다. ADR 0008은 `.cargo/`를 통째로 막았다. 정하지 않은 것: 목록 파일의 형식과 위치, 100% 대상의 범위, 기준 커밋을 누가 어떻게 주는가, 테스트를 무엇으로 식별하는가, 변이 범위, 차단을 어디에 거는가(S-14). 첫 구현의 적대적 리뷰에서 재현된 우회(고아 테스트 파일, 기준 트리 빌드 캐시 오염, 기본값 `HEAD`, 테스트 모듈의 도우미·`use`·`macro_rules!` 바꿔치기, `cargo run`까지 막는 runner, 경로 이름에만 걸린 100%, 테스트만 고친 변경의 변이 0개)를 이 판에서 닫았다.

## 실측

환경: macOS 26 (Apple Silicon), Rust 1.99.1(`llvm-tools` component), cargo-nextest 0.9.151, cargo-llvm-cov 0.9.1, cargo-mutants 27.1.0(둘 다 `cargo install --locked`), git 2.45.2.

| 항목 | 명령 | 결과 |
|---|---|---|
| S-14: nextest와 cargo `runner` | `.cargo/config.toml`에 `target.aarch64-apple-darwin.runner = ["/usr/bin/sandbox-exec", "-p", …]`, `cargo nextest run` | nextest가 runner를 썼다. 외부 연결 `PermissionDenied`, 루프백 통과. 하지만 같은 runner가 `cargo run`에도 걸려 개발 중 앱과 `toss-smoke`가 외부 네트워크를 못 쓴다 |
| nextest wrapper script | `.config/nextest.toml`에 `experimental = ["wrapper-scripts"]`, `[scripts.wrapper.network-sandbox]`(`command` 배열, `target-runner = "ignore"`), `[[profile.default.scripts]] filter = "all()"` | 0.9.151에서 동작. `CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER=/usr/bin/env`를 줘도 통과. `NEXTEST_PROFILE=default-miri`는 wrapper 없이 돈다(그래서 `justfile`이 지운다) |
| 대조군 | 같은 테스트 바이너리를 wrapper 없이 직접 실행 | 차단 테스트 2개 실패(`ConnectionRefused`) |
| DNS | 프로필에 `(deny network-outbound (remote unix-socket (path-literal "/private/var/run/mDNSResponder")))` | `example.com` 해석 실패(`nodename nor servname provided`), `localhost`는 `::1`로 해석 |
| 중첩 | `sandbox-exec -p P sandbox-exec -p P echo` | 같은 프로필이면 동작 |
| `#[coverage(off)]` | 안정판 `rustc`, feature 켜기, `RUSTC_BOOTSTRAP=1 -F unstable_features` | E0658, E0554, "use of an unstable feature" |
| 테스트 0개 | 빈 대상 crate에 `cargo llvm-cov nextest` | "no tests to run"(종료 4). `--no-tests=pass`여도 "could not load coverage information" |
| 영역 커버리지 | `cargo llvm-cov report --json` | 함수마다 `regions`(`[줄, 열, 끝 줄, 끝 열, 횟수, 파일, 펼친 파일, 종류]`). 한 줄 `if flag { 1 } else { 2 }`에서 `pick(true)`만 부르면 줄은 실행됐고 else 영역이 0 |
| 변이 예제 | 약한 테스트, `--cap-lints=true --test-tool=nextest` | missed 2, caught 5. `--cap-lints` 없이는 missed 1, unviable 4 |
| `--file`과 설정 | cargo-mutants 27.1.0 `options.rs` | `--file`은 `mutants.toml`의 `examine_globs`와 합집합이다. 범위를 좁히려면 `--no-config`가 필요하다 |
| grafts | `.git/info/grafts`로 무관한 커밋을 부모로 | `git merge-base --is-ancestor`가 참이 됐다. `--no-replace-objects`로는 꺼지지 않고 `GIT_GRAFT_FILE=/dev/null`이면 꺼졌다 |
| 소요 | `just base=… check`, `check-full` | 결과 절 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 테스트 네트워크 차단은 nextest wrapper script다. `.config/nextest.toml`이 `/usr/bin/sandbox-exec -p <프로필>`을 `target-runner = "ignore"`로 모든 테스트(`filter = "all()"`, macOS)에 씌운다. 루트 `.cargo/config.toml`은 두지 않는다 | `cargo run`과 개발 중 앱은 막지 않고 테스트만 막는다. `ignore`라 환경 변수 runner가 끼어들지 못한다 | cargo `runner`(첫 판) | harness.md 6.2, R-09 6.4, 실측 |
| 프로필 문자열은 `source_check/cargo_config.rs`의 `SANDBOX_PROFILE` 한 곳에 있다. `source-check`가 `.config/nextest.toml`을 그 상수로 만든 기대 표와 키·값까지 비교해, 다른 키(다른 프로필, `overrides`, `default-filter`, `retries`, `setup` 스크립트, `test-groups`, `store` 등)는 `nextest-config:key`, 다른 값(필터 좁히기, 플랫폼 빼기, 규칙 추가, 프로필 변경, `PATH`의 `sandbox-exec`)은 `nextest-config:value`, 빠진 키는 `nextest-config:missing`, 저장소 규칙은 `file:nextest-config`로 실패시킨다 | 차단을 줄이는 길이 모두 설정 변경이다 | 금지 키 목록 | ADR 0008 |
| 프로필은 외부 연결과 mDNSResponder Unix 소켓(DNS)을 막고 루프백·그 밖의 Unix 소켓을 연다 | 해석을 막으면 이름으로 외부에 닿는 코드가 연결 전에 실패한다 | DNS 허용(R-15) | 브리프 |
| `.cargo/`는 `mutants.toml` 하나만 허용하고(`test_tool = "nextest"`, `cap_lints = true`, `examine_globs` 세 키), `config.toml`을 포함한 그 밖의 항목은 `file:cargo-config`다 | ADR 0008의 원칙으로 되돌린다 | - | ADR 0008 |
| `justfile`이 `NEXTEST_PROFILE`, `NEXTEST_RETRIES`, `NEXTEST_FLAKY_RESULT`, `NEXTEST_NO_TESTS`, `NEXTEST_USER_CONFIG_FILE`, `NEXTEST_TEST_THREADS`, `CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER`, `CARGO_BUILD_TARGET`를 지운다 | 내장 프로필 `default-miri`는 wrapper 없이 돈다 | - | harness.md 1.1 |
| 커버리지: `crates/domain/src`, `crates/order-execution/src`, `crates/application/src/order_gate`(와 `domain`의 `signal`·`threshold`·`margin`·`calibration`·`order`·`validate`·`outcome`)가 100%다. 95%는 `harness/coverage/policy.toml`의 `relaxed`에 적은 `domain` 경로뿐이고 처음에는 비어 있다. `relaxed`는 `domain` 밖이거나 100% 모듈과 겹치면 정책 읽기가 실패한다. 가장 긴 경로가 이긴다 | 새 파일(`sizing.rs`)이 이름 때문에 95%만 받는 일이 없다 | 경로 이름으로 100% 지정(첫 판) | harness.md 6.1, 브리프 |
| 100% 파일은 줄(`coverage:uncovered-line`)과 실행된 줄 안의 놓친 코드 영역(`coverage:uncovered-region`) 모두 예외 밖이면 실패다. 정책 경로 안에 본문 있는 함수가 있는데 보고서에 없는 파일은 `coverage:unmeasured-file`, 측정 패키지에 테스트가 하나도 없으면 그런 파일이 `coverage:untested-file`이다. 측정은 `cargo llvm-cov nextest --no-report` 뒤 `report --lcov`와 `report --json`이다 | 한 줄 분기는 줄 커버리지로 보이지 않는다. 모듈 트리에 닿지 않는 파일은 측정되지 않는다 | 분기 커버리지(nightly) | harness.md 6.3, 브리프 |
| 커버리지 예외는 `harness/coverage/exceptions.toml`의 `[[exception]]`(`path`, `function`, `reason`)이고 함수의 `fn`부터 본문 끝까지 줄·영역을 뺀다. 없는 파일·함수, 같은 이름 여러 개면 실패. 처음에는 비어 있다 | 6.3 "파일과 함수 단위" | 줄 번호 | harness.md 6.3 |
| `test-diff`·`mutants --in-diff`의 기준은 인자로만 받고 `HEAD` 또는 40자리 커밋 ID만 받으며, HEAD의 조상이어야 한다. `just check`·`check-full`은 `base` 변수가 비어 있으면 첫 단계 `require-base`에서 안내와 함께 실패한다(`just base=<HEAD|커밋 ID> check`) | 기본값이 있으면 부르는 쪽이 잊어도 통과한다. 브랜치 이름은 옮길 수 있다(ADR 0011) | 기본값 `HEAD`(첫 판), 브랜치 `merge-base` | harness.md 7.7, ADR 0011 |
| git은 `--no-replace-objects`, `GIT_GRAFT_FILE=/dev/null`, `core.fsmonitor=false`로 부르고 효과가 있는 리다이렉트 변수 13개(`GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`, `GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_COMMON_DIR`, `GIT_SHALLOW_FILE`, `GIT_CONFIG_GLOBAL`, `GIT_CONFIG_SYSTEM`, `GIT_CONFIG_COUNT`, `GIT_CONFIG_PARAMETERS`, `GIT_DIFF_OPTS`, `GIT_ATTR_SOURCE`)를 지운다. 변수마다 "그냥 git에서는 결과가 바뀌고 하네스 git에서는 그대로"인 사례가 있다 | 원소를 빼면 그 사례가 실패한다. `GIT_NAMESPACE`·`GIT_CEILING_DIRECTORIES`·`GIT_REPLACE_REF_BASE`·`GIT_EXTERNAL_DIFF` 등은 이 하네스의 git 호출에서 효과를 만들지 못해 목록에서 뺐다 | - | 브리프 |
| `test-diff` 소스 비교: 기준 커밋의 git 객체(`ls-tree`, `cat-file --batch`)와 작업 사본(`ls-files --cached --others --exclude-standard`)을 읽어 같은 방식으로 정적으로 푼다. 작업 공간 `members`의 각 패키지에서 lib·bin·`tests/` 루트를 찾고 `mod` 선언을 따라 모듈 트리를 만든다. 테스트 ID는 nextest와 같은 `<binary-id> <모듈 경로>::<함수>`다. 지문은 둘러싼 모듈 선언의 속성·매크로 이름, 파일 내부 속성, 함수 속성과 토큰이다. 없으면 `removed`, 다르면 `modified`, 모듈 트리에 닿지 않는 `.rs` 파일(`harness/` 밖)에 `#[test]`가 있으면 `unreachable` | `cfg`가 `cfg(test)`뿐이고 `#[path]`가 금지라 정적으로 풀 수 있다. 같은 이름의 인라인 모듈로 바꾸고 원래 파일을 버리는 것이 `unreachable`로 걸린다 | 파일 경로 ID(첫 판) | harness.md 7.7, 브리프 |
| 테스트 모듈(`cfg(test)`이거나 테스트를 가진 모듈과 그 아래)의 비테스트 항목(`use`, 함수, `const`, `static`, 타입, `impl`, `macro_rules`, `extern crate`, 하위 `mod`)을 (종류, 이름) 키의 지문으로 둔다. 기존 항목이 사라지면 `support-removed`, 바뀌면 `support-modified`. 새 항목은 허용하되, 그 모듈의 기존 테스트·도우미가 쓰는 식별자를 새로 묶는 항목(가져온 이름·별칭 포함), 새 글롭 `use`, 표준 매크로 이름(`assert`, `assert_eq`, `assert_ne`, `debug_assert*`, `matches`, `panic`, `unreachable`, `todo`, `unimplemented`, `vec`, `format`, `write`, `writeln`, `println`, `eprintln`, `dbg`)의 `macro_rules`는 `shadowing`이다. 새 `#[test]`는 이 집합을 바꾸지 않는다 | 도우미를 약하게 하거나 `use`를 지우고 가짜 함수를 넣거나 `assert_eq!`를 가리는 길을 막는다. 묶은 `use`를 고치면 옛 `use`가 사라진 것으로 보이므로 새 이름은 새 `use` 줄로 더한다 | 테스트 함수만 비교(첫 판) | 브리프 |
| 목록 비교: 기준 테스트 이름(정적)이 현재 `cargo nextest list --workspace --locked`에 모두 있어야 하고(`not-listed`), 현재 개수가 기준보다 적으면 `count-decreased`. 기준 트리는 빌드하지 않고 `target/` 아래의 어떤 캐시도 판정에 쓰지 않는다 | 기준 트리 빌드 캐시를 오염시켜 통과하는 길(리뷰 재현)을 없앤다 | 기준 트리를 풀어 `nextest list`(첫 판) | 브리프 |
| 변이 대상은 `.cargo/mutants.toml`의 `examine_globs`(`domain`, `order-execution`, `application/src/order_gate*`)이고 xtask가 `--no-config --cap-lints=true --test-tool=nextest --file <glob>…`으로 부른다. `--in-diff <기준>`은 변경(작업 사본 포함)이 대상 crate 폴더(테스트 파일·`tests/` 포함)를 하나라도 건드리면 그 crate의 대상 전체를 변이한다. `check-full`은 변경분 뒤에 전체(`mutants-all`)도 돈다(이 범위와 전체 실행은 ADR 0042가 대체했다). 생존은 `harness/mutants/survivors.toml`에 있어야 하고 시간 초과는 실패다 | 테스트만 약화하면 변경분 변이가 0개였다. 대상 crate가 아직 거의 비어 있어 전체 실행이 싸다 | 변경 줄만(`--in-diff` 파일, 첫 판) | harness.md 6.4, 브리프 |
| `rust-toolchain.toml`의 `components`에 `llvm-tools`를 더한다 | 새 사본에서도 커버리지가 돈다 | 수동 설치 | 메인 승인 |

## 결과

- 기존 테스트 변경: 이 판은 첫 판(f6123fe)의 하네스 자체 테스트 일부를 바꿨다. `committed_weakening_…_but_not_against_head`는 메인이 승인했다. 나머지는 완료 기준이 바꾸게 한 것이다: ID 형식과 `unreachable`·`removed`가 더해진 `dropping_a_test_module_declaration_…`, `replacing_a_disabled_test_…`, `replace_refs_…`, `.cargo/config.toml` 금지와 nextest 설정으로 바뀐 `cargo_config_tests`의 세 테스트·도우미, `domain` 100%로 바뀐 커버리지 상수 `DOCUMENTED_THRESHOLDS`·`RULES`. `just base=f6123fe… check`의 `test-diff`가 이것들을 보고하므로 사용자 승인이 필요하다. 533d077 기준으로는 모두 새 테스트다.
- 원리상 놓치는 것(`test-diff`): 테스트가 부르는 다른 crate의 도우미·`testkit`·구현 코드의 변경, 예제 데이터 파일(`harness/`는 보호 경로), `#[rstest]`·`#[test_case]` 같은 다른 테스트 속성(그 의존성을 허용할 때 정한다), 매크로 안에서 만들어지는 모듈, 기준 커밋 자체에 들어 있는 약화, 부르는 쪽이 기준을 잘못 주는 것, 현재 빌드가 낡은 경우(mtime을 되돌린 소스, 아래 H-14·H-25), SHA-1 충돌. `shadowing`은 이름 비교라 별칭 없이 다른 경로의 같은 이름을 가져오는 것은 이름이 겹칠 때만 잡는다.
- 원리상 놓치는 것(차단): 루프백 포트 단위 제한은 SBPL로 표현할 수 없다. 테스트는 로컬의 다른 서비스(루프백)에 닿을 수 있다. `sandbox-exec`는 폐기 예정이다(자체 테스트가 먼저 실패한다). wrapper script는 nextest의 실험 기능이다.
- 원리상 놓치는 것(커버리지·변이): 분기 커버리지, 정책 경로 밖에 둔 신호·주문 계산 코드, cargo-mutants가 만들지 않는 변이, `#[cfg(test)]` 모듈 안.
- `check-full`은 첫 단계가 권고 DB 받기라 `base` 없이 부르면 받은 뒤 `require-base`에서 실패한다(기존 테스트가 `check-full: advisory-db-fetch check` 머리줄을 고정한다).
- 뒤 작업으로 넘긴 것:
  - H-14: 커밋 훅은 `just base=HEAD check`, 푸시 훅은 푸시 전 원격 SHA를 기준으로 `check-full`.
  - H-25: `SubagentStart`가 브리프의 기준 커밋 SHA를 작업 사본 밖 상태 파일에 기록하고 `SubagentStop`이 그 SHA로 `just check`를 돈다. 그 SHA가 HEAD의 조상이 아니면 실패. 새 기준은 시작 시점의 기준 브랜치 끝이거나 상태가 `passed`인 앞 단계의 대상 SHA여야 한다("막힘"으로 끝난 단계의 SHA를 기준으로 쓰면 그 안의 약화가 세탁된다).
  - H-14·H-25: 훅의 `just check`는 훅이 소유한 작업 사본 밖 target 폴더에서 돌거나 먼저 작업 공간 crate를 정리한다(소스를 고친 뒤 mtime을 되돌리면 cargo가 다시 빌드하지 않는다. 그 예제가 실패해야 한다).
  - H-35: `sandbox-exec`는 바깥과 프로필이 글자까지 같을 때만 중첩된다. Claude Code 샌드박스 안에서 `just check`의 nextest가 통과하게 하거나, 테스트 차단을 샌드박스 네트워크 정책으로 대체하는 ADR.
  - H-16: 밤 작업에서 `just mutants-all`, 깨끗한 빌드, 푸시 전 SHA 기준. Linux는 wrapper가 macOS에만 걸리므로 `--network none`으로 따로 막는다.
  - H-10: nextest 버전 고정(wrapper script가 실험 기능이다).
- 바뀌면 다시 볼 조건: macOS가 `sandbox-exec`를 없앨 때, nextest의 wrapper script가 정식이 되거나 형식이 바뀔 때, cargo-mutants의 결과 파일·종료 코드·`--file` 병합이 바뀔 때, cargo-llvm-cov의 JSON 형식이 바뀔 때, 테스트 속성 crate를 허용할 때.
