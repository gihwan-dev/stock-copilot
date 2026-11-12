# ADR 0013: Rust 테스트 하네스 (커버리지, `test-diff`, 변이 테스트, 테스트 네트워크 차단)

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-11(Rust 쪽), H-29(Rust 쪽), H-17, H-18
- 반영한 문서: 없음 (harness.md 1.1, 1.2, 3.5, 6.1~6.4, 7.7, R-09 Q6·Q7·Q9, R-15 2.7, ADR 0005·0006·0008·0011을 따르고 문서가 정하지 않은 곳만 여기 적는다)

## 맥락

harness.md 6.1은 파일별 줄 커버리지 기준(`domain` 95%, `order-execution`·주문 게이트·`domain` 신호·주문 계산 모듈 100%)을, 6.3은 보호된 예외 목록 밖의 실행되지 않은 줄을 실패로, 6.4는 `cargo-mutants`의 대상과 생존 허용 목록을, 7.7은 기준 커밋 대비 기존 테스트의 삭제·변경과 `nextest list` 개수 감소를, 6.2는 루트 `.cargo/config.toml`의 `runner`로 `sandbox-exec` 네트워크 차단을 정했다. ADR 0008은 `.cargo/`를 통째로 막았고 이 작업이 허용을 연다고 적었다. 정하지 않은 것: 기준·예외·허용 목록의 파일 형식과 위치, 100% 대상 모듈의 이름, 기준 커밋을 누가 어떻게 주는가, 변이 테스트 대상 표현, `runner`의 정확한 값, 테스트가 하나도 없을 때의 커버리지, nextest가 `runner`를 쓰는가(S-14).

## 실측

환경: macOS 26 (Apple Silicon), Rust 1.99.1, cargo-nextest 0.9.151, cargo-llvm-cov 0.9.1(`cargo install --locked`), cargo-mutants 27.1.0(`cargo install --locked`), `rustup component add llvm-tools`.

| 항목 | 명령 | 결과 |
|---|---|---|
| S-14: nextest와 cargo `runner` | `.cargo/config.toml`에 `target.aarch64-apple-darwin.runner = ["/usr/bin/sandbox-exec", "-p", …]`, `cargo nextest run` | nextest가 `info: using target runner … defined by target.aarch64-apple-darwin.runner`를 출력하고 그 runner로 실행했다. `1.1.1.1:443` 연결이 `PermissionDenied`(os error 1), 루프백·Unix 소켓 가짜 서버는 통과 |
| 대조군 | 같은 테스트 바이너리를 runner 없이 직접 실행 | `outbound_connections_fail_with_permission_denied`, `child_processes_inherit_the_network_block` 두 개가 실패(연결 오류가 `ConnectionRefused`) |
| 자식 프로세스 | runner 안에서 `/usr/bin/nc -z 1.1.1.1 443` | `connectx … failed: Operation not permitted` |
| 겹친 샌드박스 | `sandbox-exec -p P sandbox-exec -p P echo` | 동작한다. 테스트 안에서 다시 cargo·nextest를 부르는 하네스 테스트도 통과 |
| 기존 테스트 | runner를 켠 채 `cargo nextest run --workspace --locked` | 101개 모두 통과(권고 DB, 예제 작업 공간 빌드 포함) |
| `#[coverage(off)]` | 안정판 `rustc`, `#![feature(coverage_attribute)]`, `RUSTC_BOOTSTRAP=1 -F unstable_features` | E0658, E0554, "use of an unstable feature". 코드로 커버리지를 빼는 길이 없다 |
| cargo-llvm-cov 표준 입력 | 터미널 없이 `cargo llvm-cov nextest` | 표준 입력을 열어 두면 멈췄다. `Stdio::null()`로 부른다 |
| 테스트 0개 | 지금의 빈 `domain`·`order-execution`·`application`에 `cargo llvm-cov nextest` | nextest가 "no tests to run"(종료 4), `--no-tests=pass`를 줘도 llvm-cov가 "could not load coverage information"으로 실패 |
| 변이 테스트 예제 | 약한 테스트(`is_positive(1)`만 확인)와 강한 테스트를 둔 예제, `--cap-lints=true --test-tool=nextest` | 7개 중 missed 2(`replace is_positive -> bool with true`, `replace > with >= in is_positive`), caught 5, 7.6초 |
| `--cap-lints` 없이 | 같은 예제, 작업 공간 린트 상속 | missed 1, caught 2, unviable 4. 생존 변이 하나가 unviable로 숨었다(R-09 Q6과 같음) |
| `dist` 없는 사본 | 예제에 `include_str!("../../dist/index.html")`을 쓰는 `src-tauri` 멤버를 두고 `dist` 없이 실행 | 통과. cargo-mutants는 변이가 있는 패키지만 빌드한다. 실제 저장소도 `dist`가 `.gitignore`에 있어 복사되지 않는다 |
| 빈 변이 대상 | 실제 저장소에서 `xtask mutants`, `--in-diff`에 빈 diff | "No mutants found", "Diff file is empty", 둘 다 종료 0 |
| `test-diff` 소요 | 실제 저장소, 기준 533d077, 작업 사본 변경 있음 | 첫 실행 50.7초(기준 트리 첫 빌드), 이후 2~10초 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 네트워크 차단은 루트 `.cargo/config.toml`의 `target.aarch64-apple-darwin.runner = ["/usr/bin/sandbox-exec", "-p", "<프로필>"]`이다. 프로필은 `(version 1)(allow default)(deny network-outbound)(allow network-outbound (remote ip "localhost:*"))(allow network-outbound (remote unix-socket))`이고 파일이 아니라 인자로 준다 | S-14에서 nextest가 runner를 쓴다. 인자 경로는 현재 폴더 기준이라 nextest가 패키지 폴더에서 실행하면 프로필 파일을 못 찾는다. 인자로 주면 보호할 파일이 하나 줄고, 절대 경로라 `PATH`의 가짜 `sandbox-exec`를 쓰지 않는다 | nextest wrapper script(실험 기능, 필요 없어짐), `-f deny-net.sb`(상대 경로) | harness.md 6.2, R-15 2.7, 실측 |
| 차단 자체 테스트는 `crates/xtask/src/network_sandbox_tests.rs`의 네 개다: 외부 IPv4·IPv6 연결이 `PermissionDenied`, 자식 `nc`도 `Operation not permitted`, 루프백 TCP 가짜 서버, Unix 소켓 가짜 서버 | "실패해야 통과"를 오류 종류로 판정해야 오프라인 환경의 `ConnectionRefused`·시간 초과와 구별된다 | 연결 실패만 확인 | harness.md 6.2, 6.8 |
| `source-check`는 루트 `.cargo/`에 `config.toml`과 `mutants.toml` 두 일반 파일만 허용한다. 그 밖의 항목, 하위 폴더, 루트 밖의 `.cargo`는 계속 `file:cargo-config`다. 허용한 두 파일도 내용 검사를 통과하지 못하면 그 파일 경로로 `file:cargo-config`가 난다 | 기존 예제(`[env]`, `[alias]`, 옛 이름 `config`, crate 안의 `.cargo`)의 기대값을 바꾸지 않는다 | 내용 문제마다 다른 저장소 규칙 이름(기존 예제 기대값이 바뀐다) | ADR 0008 |
| `config.toml`은 `target.aarch64-apple-darwin.runner` 키 하나만 있고 값이 위 배열과 글자까지 같아야 한다. 다른 최상위 키(`env`, `alias`, `build`, `unstable`, `include`, `net`, `source`, `profile` 등), 다른 triple·`cfg(…)` target, triple 표 안의 다른 키(`rustflags`, `linker`)는 `cargo-config:key:<경로>`, 값이 다르면 `cargo-config:value:…`, 없으면 `cargo-config:missing:…`, 읽을 수 없으면 `cargo-config:unparsable`이다 | 허용 목록 방식이라 cargo가 키를 새로 더해도 막힌다. 기대값은 xtask 코드(보호 경로)에 있다 | 금지 키 목록 | harness.md 7.1, ADR 0008 |
| `mutants.toml`은 `test_tool = "nextest"`, `cap_lints = true`, 비어 있지 않은 문자열 배열 `examine_globs` 세 키만 허용한다. `exclude_globs`, `exclude_re`, `examine_re`, `skip_calls`, `error_values`, `additional_cargo_args`, `additional_cargo_test_args`, `timeout_multiplier` 등은 `cargo-mutants:key:…`다. `examine_globs`의 내용은 테스트 `mutation_targets_are_the_documented_modules`가 harness.md 6.4의 대상과 같은지 고정한다 | 나머지 키는 모두 변이를 줄이거나 테스트를 건너뛰는 길이다 | 키 제한 없음 | harness.md 6.4 |
| 변이 대상은 `crates/domain/src/**/*.rs`, `crates/order-execution/src/**/*.rs`, `crates/application/src/order_gate.rs`, `crates/application/src/order_gate/**/*.rs`다. 주문 게이트 모듈 이름은 `order_gate`로 정한다 | 문서에 모듈 이름이 없다. 정한 이름 밖에 게이트를 두면 리뷰에서 본다 | `application` 전체 | harness.md 6.4 |
| `xtask mutants [--in-diff <기준>]`가 `cargo mutants --cap-lints=true --test-tool=nextest --no-shuffle`을 부르고 `missed.txt`·`timeout.txt`를 읽는다. 생존 변이는 `harness/mutants/survivors.toml`의 `[[survivor]]`(`file`, `mutant`(줄 번호 없는 설명), `reason`)에 있으면 통과한다. 시간 초과는 목록과 상관없이 실패, unviable은 통과다. 목록의 `file`은 있어야 한다. 목록은 처음에 비어 있다 | 줄 번호는 코드가 바뀌면 움직인다. unviable은 타입이 변이를 표현하지 못한다는 증거다(6.4) | 종료 코드만 보기(허용 목록을 쓸 수 없다) | harness.md 6.4 |
| `just check-full`은 `check` 뒤에 `mutants-diff {{base}}`(기준 커밋 대비 변경분, `--in-diff`)를 돈다. 전체 실행은 `just mutants-all`이고 밤 CI(H-16)가 부른다 | harness.md 1.2·6.4: 푸시 훅은 변경분, 밤은 전체. 기존 테스트가 `check-full: advisory-db-fetch check` 머리줄을 고정한다 | `check-full`에서 전체 | harness.md 1.2, 6.4 |
| 커버리지 기준은 `harness/coverage/policy.toml`이다. `packages`(측정할 패키지)와 `[[threshold]]`(`path`, `percent`, `reason`). 경로 P는 파일 P, `P.rs`, `P/` 아래 모든 파일에 걸리고 여러 개가 걸리면 가장 높은 기준을 쓴다. 100% 대상은 `crates/order-execution/src`, `crates/application/src/order_gate`, `crates/domain/src/{signal,threshold,margin,calibration,order,validate,outcome}`다 | 모듈이 아직 없으므로 경로로 미리 적는다. 없는 경로는 기준을 더할 뿐 예외를 열지 않으므로 ADR 0008의 "있는 경로만" 규칙이 필요 없다 | 모듈이 생길 때 추가(빠뜨리기 쉽다) | harness.md 6.1 |
| `xtask coverage-check`는 `cargo llvm-cov nextest --locked --lcov`를 정책의 패키지마다 `--package`로 한 번 돌리고 LCOV의 `DA` 줄로 판정한다. 100% 파일은 예외 밖의 실행되지 않은 줄마다 `coverage:uncovered-line`, 그 밖은 (실행된 줄 + 예외 줄)이 기준 미만이면 `coverage:below-threshold`. 측정 패키지에 테스트가 하나도 없으면 llvm-cov를 부르지 않고, 기준 경로 안에서 본문 있는 함수가 하나라도 있는 파일을 `coverage:untested-file`로 실패시킨다 | `--fail-under-file-lines`는 기준이 하나뿐이고 예외 목록을 모른다. 테스트 0개면 llvm-cov가 실패하므로 빈 crate와 테스트 없는 코드를 구별해야 한다 | `--fail-under-file-lines` 두 번 | harness.md 6.1, 6.3, 실측 |
| 커버리지 예외는 `harness/coverage/exceptions.toml`의 `[[exception]]`(`path`, `function`, `reason`)다. 함수는 그 파일에서 `fn <이름>`을 토큰으로 찾아 `fn`부터 본문 끝 줄까지를 예외로 친다. 파일이 없거나, 함수가 없거나, 같은 이름이 여러 개면 목록 읽기가 실패한다. 처음에는 비어 있다 | "파일과 함수 단위"(6.3). 줄 번호로 적으면 코드가 움직일 때마다 바뀐다 | 줄 번호, LLVM 함수 이름(맹글링) | harness.md 6.3 |
| 커버리지 검사는 `just check`에서 nextest 다음에 돈다 | harness.md 1.1이 `check`에 커버리지를 넣었다 | `check-full` | harness.md 1.1 |
| `xtask test-diff <기준>`의 기준은 인자로만 받고 `HEAD` 또는 40자리 커밋 ID만 받는다. 브랜치·태그·`HEAD~1`·짧은 ID는 실패한다. `just check`은 `base` 변수(기본 `HEAD`)를 넘기고 부르는 쪽이 `just base=<커밋 ID> check`로 바꾼다. 종료 훅은 브리프의 기준 커밋, 푸시 훅과 CI는 푸시 전 원격 커밋 ID를 준다 | 브랜치 이름은 에이전트가 만들거나 옮길 수 있다(ADR 0011 리뷰). 커밋 ID는 내용 주소라 바꿀 수 없다. 기본값 `HEAD`는 커밋 훅에서 "이번 커밋의 변경"과 같다 | 브랜치 `merge-base`, 환경 변수, 보호 파일에 커밋 ID 적기(main이 움직일 때마다 사람이 고쳐야 한다) | harness.md 7.7, ADR 0011 |
| git은 `--no-replace-objects`, `core.fsmonitor=false`로 부르고 `GIT_DIR`·`GIT_INDEX_FILE`·`GIT_OBJECT_DIRECTORY`·`GIT_CONFIG*`·`GIT_REPLACE_REF_BASE` 등 저장소·객체·설정을 바꾸는 환경 변수를 지운다. 기준 쪽 파일은 `ls-tree`와 `cat-file --batch`로 객체에서 직접 읽는다 | `git replace`로 기준 커밋을 다른 커밋으로 바꿔 보이게 할 수 있다(`replace_refs_do_not_change_the_base`). `git archive`는 `.git/info/attributes`의 `export-ignore`로 파일을 뺄 수 있다 | `git archive`, `git show` | 리뷰 대비 |
| 소스 비교: 기준의 `.rs` 파일(`harness/` 아래 제외)에서 `#[test]`·`#[<경로>::test]` 함수를 `파일 경로::인라인 모듈::함수 이름`으로 찾고, 지문은 둘러싼 모듈의 속성과 매크로 이름, 파일의 내부 속성, 함수의 속성, 함수 토큰(가시성부터 본문까지)이다. 작업 사본에 같은 ID가 없으면 `test-diff:removed:<ID>`, 지문이 다르면 `test-diff:modified:<ID>`. 작업 사본은 `git ls-files --cached --others --exclude-standard`의 파일을 디스크에서 읽는다(커밋하지 않은 변경 포함) | 이름 바꾸기·모듈 옮기기는 삭제로, `assert_eq!`→`assert!(true)`·조기 반환·`#[cfg(any())]`·`#[should_panic]`·`#[ignore]`·둘러싼 `mod`의 `cfg`·`#![cfg]`·`proptest!` 입력 범위 축소는 변경으로 걸린다. 토큰 비교라 rustfmt 줄바꿈은 통과한다 | 줄 단위 diff(형식 변경이 실패한다) | harness.md 7.7 |
| 목록 비교: 작업 사본이 기준 커밋의 파일과 바이트까지 같지 않으면, 기준 트리를 `target/test-diff/tree`에 객체에서 써서(바뀐 파일만 다시 써서 증분 빌드) `cargo nextest list --workspace --locked`(대상 폴더 `target/test-diff/target`)를 돌리고 작업 사본의 목록과 비교한다. 기준에 있고 지금 없는 이름마다 `test-diff:not-listed`, 개수가 줄면 `test-diff:count-decreased:<기준>-><현재>`. 기준 트리에 `apps/desktop/dist`가 없으면 작업 사본의 것을 복사한다 | 개수만 보면 테스트 모듈 선언을 지우고 새 테스트를 같은 수만큼 더하는 것을 놓친다(`replacing_a_disabled_test_with_a_new_one_still_fails`). 같음 판정을 `git status`로 하면 `assume-unchanged`로 속일 수 있어 내용을 직접 비교한다. `generate_context!`가 `dist`를 요구한다(ADR 0006) | 개수만, `git status` | harness.md 7.7, 실측 |
| `test-diff`는 `just check`에서 nextest 다음에 돈다. TS 테스트 블록과 `vitest list`는 `test_diff`의 `in_scope`·`compare_lists`에 같은 방식으로 더할 수 있게 비워 둔다 | harness.md 1.1·7.7. TS는 이 작업 범위 밖이다 | - | 브리프 |
| `just`의 어떤 레시피도 `cargo test`·`cargo bench`를 부르지 않는다(`no_recipe_runs_rust_tests_outside_nextest`). 커버리지는 `llvm-cov nextest`, 변이는 `--test-tool=nextest`다 | harness.md 1.1 | - | harness.md 1.1 |

## 결과

- 루트 `.cargo/config.toml`의 runner는 `cargo run`에도 걸린다. `cargo run`으로 띄운 앱(개발 중 `tauri dev` 포함)과 xtask도 외부 네트워크를 못 쓴다. 지금 xtask는 네트워크가 필요 없다. 앱을 개발 중 실제 토스에 붙여야 할 때(U-01 뒤) 다시 본다.
- xtask가 runner 안에서 돌므로 `test-diff`의 기준 트리 빌드와 `mutants`는 로컬 레지스트리 캐시에 있는 crate로만 빌드된다. 기준 커밋에만 있고 캐시에 없는 crate가 있으면 실패한다. 깨끗한 CI(H-16)에서는 `test-diff` 전에 기준 잠금 파일로 `cargo fetch`를 돌려야 한다.
- `rust-toolchain.toml`의 `components`에 `llvm-tools`가 없다(보호 파일이라 이 작업에서 바꾸지 않았다). 새 환경에서는 `rustup component add llvm-tools --toolchain 1.99.1`이 필요하고, 없으면 `coverage-check`가 안내와 함께 실패한다. 사용자 승인으로 `components`에 더하는 것을 권한다.
- cargo-llvm-cov와 cargo-mutants는 `cargo install --locked`로 설치한다. 버전 고정 방법은 harness.md 1.3의 다른 도구와 같이 H-10·H-16에서 정한다.
- `test-diff`가 원리상 놓치는 것: 테스트가 부르는 도우미 함수·상수·예제 데이터 파일·구현 코드의 변경(본문은 같고 의미가 바뀐다), 테스트 모듈의 `use` 변경으로 이름이 다른 것을 가리키게 하는 것, 기준 쪽에서 이미 파싱되지 않던 파일, `harness/` 아래 예제(보호 경로로 막는다), 부르는 쪽이 기준 커밋을 잘못 주는 것(종료 훅·CI가 맡는다). 기준 커밋 자체가 약화를 담고 있으면 그것은 기준이다. SHA-1 충돌로 객체를 위조하는 것은 다루지 않는다. 구현을 베낀 테스트는 변이 테스트가 맡는다(harness.md 7.7).
- 커버리지가 원리상 놓치는 것: 분기 커버리지(안정판에서 쓸 수 없다, 6.3), 한 줄에 여러 식이 있을 때 일부만 실행된 줄, 정책 경로 밖에 둔 신호·주문 계산 코드. 테스트가 하나도 없을 때의 `untested-file`은 줄 단위가 아니라 파일 단위다.
- 변이 테스트가 원리상 놓치는 것: cargo-mutants가 만들지 않는 변이, `#[cfg(test)]` 모듈과 테스트 함수 안(도구가 건너뛴다), `examine_globs` 밖의 파일. 진단 출력을 비교하는 `trybuild` 테스트를 nextest 필터로 빼는 일(6.4)은 그 테스트가 생길 때 `mutants.toml` 허용 키와 함께 정한다.
- 하네스 자체 테스트가 `just check`에서 cargo-llvm-cov와 cargo-mutants를 부른다(예제 작업 공간 하나씩, 각각 약 5초, 7초).
- 바뀌면 다시 볼 조건: macOS가 `sandbox-exec`를 없앨 때(자체 테스트가 먼저 실패한다), nextest가 runner 처리를 바꿀 때, cargo-mutants의 결과 파일 형식이나 종료 코드가 바뀔 때, cargo-llvm-cov가 테스트 0개를 다르게 다룰 때, 신호·주문 계산 모듈 이름이 정해질 때.
