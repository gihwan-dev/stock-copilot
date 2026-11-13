# ADR 0020: CI 작업 흐름, 보호 경로 서명 검사, 리뷰 기록 검사

- 상태: 제안
- 날짜: 2026-11-13
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-16, H-26, H-27 (H-28, H-20과 맞물림)
- 반영한 문서: 없음(이 ADR만 추가. 문서에 넣을 문장은 "메인이 반영할 문서 변경"에 모았다)

## 맥락

- 원격 저장소 `github.com/gihwan-dev/stock-copilot`가 생겼다. 공개 저장소라 표준 러너가 무료이고 브랜치 규칙 세트를 쓸 수 있다. ADR 0003과 harness.md 7.6은 "무료 요금제 비공개 저장소라 규칙 세트를 못 쓴다"를 전제로 CI를 사후 탐지로만 정했다. 그 전제가 바뀌었다.
- harness.md 1.2는 푸시마다 Linux, `main`(이 저장소는 `master`) 푸시와 밤에 macOS, 밤에 Linux 전체를 정했다. ADR 0013·0015는 깨끗한 빌드, 푸시 전 SHA 기준, 깨끗한 checkout의 훅 설치 확인, 병합·fast-forward 범위를 H-16에 넘겼다. ADR 0017은 기준 커밋의 허용 목록 판정, 훅 없이 만든 커밋, 의존성 절만 바꾸는 매니페스트를 H-26에 넘겼다.
- 서명 키(H-28)는 아직 없다. 지금까지 서명 없는 커밋이 보호 경로를 만들었다.

## 실측

환경: macOS 26(Apple Silicon), Docker(colima, linux/arm64) `ubuntu:24.04`, Rust 1.99.1, just 1.59.0, cargo-nextest 0.9.151, cargo-deny 0.20.2, cargo-llvm-cov 0.9.1, cargo-mutants 27.1.0, lefthook 2.2.2, Node 24.22.0, pnpm 12.18.0.

| 항목 | 명령 | 결과 |
|---|---|---|
| Linux에서 작업 공간 전체 | `cargo clippy --workspace --all-targets --locked` | `glib-sys` 빌드 스크립트 실패(`glib-2.0` 없음). `src-tauri`의 `tauri`(wry)가 원인 |
| 제외 목록 밖 crate | `cargo clippy --workspace --exclude src-tauri --exclude e2e-app --exclude platform-macos --exclude secrets-keychain --all-targets` | 통과 |
| 도구 없이 Linux 테스트 | 같은 범위로 `cargo nextest run` | 212개 중 47개 실패(`cargo deny`·`llvm-cov`·`mutants`·`node`·`lefthook` 없음, `--offline` 레지스트리 비어 있음) |
| 도구를 고정 버전으로 설치하고 `cargo fetch` 뒤 | 같음 | 6개 실패: `network_sandbox_tests` 3, `nested_sandbox_tests` 1(`sandbox-exec` 없음), `rust_lints_tests::violation_examples_*` 2(`rust_decimal =1.43.0`이 레지스트리 캐시에 없음) |
| `rust_decimal`·`num-traits` 미리 받기 뒤 | 같음 | 두 `rust_lints_tests` 통과 |
| 네트워크 이름공간 | 특권 컨테이너에서 `node harness/ci/linux/offline.mjs` (`sudo unshare --net` → `ip link set lo up` → `setpriv`로 원래 사용자) | `curl example.com` 실패(6), 루프백 테스트 포함 `just linux-test` 209개 통과(10개 제외 필터로 건너뜀), 만든 파일의 소유자는 원래 사용자 |
| Linux 작업 묶음 | 위 컨테이너에서 `just linux-check` | 통과(형식, clippy, deny, xtask 검사 7개, 작업 흐름 정책, 리뷰 기록) |
| 서드파티 액션 SHA | `gh api repos/<액션>/git/ref/tags/<태그>` | checkout v7.0.1 `3d3c42e…`, setup-node v7.1.0 `949feb2…`, cache v6.1.0 `55cc834…`, taiki-e/install-action v2.90.3 `cfdb7ed…`(모두 commit 객체) |
| install-action의 고정 버전 목록 | `manifests/<도구>.json@v2.90.3` | just 1.59.0, nextest 0.9.151, deny 0.20.2, llvm-cov 0.9.1은 Linux arm64·macOS arm64 해시가 있다. cargo-mutants 27.1.0은 x86_64만 있다 |
| lefthook 2.2.2 해시 | 릴리스의 `lefthook_checksums.txt`와 내려받은 파일의 `shasum -a 256` | Linux aarch64 `a4d4bf78…`, Linux x86_64 `4fbf89eb…`, macOS arm64 `049c6485…`(내려받아 같음을 확인) |
| 규칙 세트 문서 | docs.github.com "Available rules for rulesets", "About protected branches" | 삭제 금지·강제 푸시 금지는 브랜치 규칙 세트에 있다. 상태 검사 필수는 "검사가 통과한 뒤 다른 브랜치로 먼저 푸시한 커밋을 병합하거나 보호 브랜치에 직접 푸시"하는 방식으로 직접 푸시에도 적용된다. 파일 경로 제한(푸시 규칙 세트)은 비공개·내부 저장소에만 있다 |

로컬 테스트: `node --test 'harness/ci/tests/*.test.mjs'` 37개 통과(서명 검사 17개는 임시 저장소와 임시 SSH 키를 쓴다), xtask 새 테스트 7개 통과.

## 결정

### 작업 흐름

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| `.github/workflows/ci.yml`(push 모든 브랜치, `master` 대상 PR)과 `nightly.yml`(매일 18:00 UTC = 03:00 KST) 두 파일. `ci.yml`: `signatures`·`linux`(Linux arm64, 항상), `macos`(macos-15, `master` 푸시만), `notify`. `nightly.yml`: `changes` → `macos`(그날 바뀌었을 때만), `linux`(전체 + 변이), `notify` | harness 1.2의 배치 그대로. 밤 macOS는 "그날 변경이 있을 때만"(1.2) | 한 파일, `workflow_dispatch` | harness 1.2 |
| 작업 흐름 파일은 YAML이 아니라 2칸 들여쓴 JSON으로 쓴다(JSON은 YAML이다). 정책 검사가 `JSON.stringify(…, 2)`와 글자까지 같은지 본다 | 의존성(YAML 파서) 없이 정확히 읽는다. 앵커·태그·주석 같은 YAML 기능으로 검사를 피하는 길이 없다. 주석 금지도 저절로 지켜진다 | YAML 라이브러리를 허용 목록에 더하기, 줄 단위 정규식 | harness 2장 |
| Linux 작업은 `just linux-check`(제외 목록 검증, `cargo fmt`, 제외한 묶음의 clippy, `deny`, xtask 검사 7개, 작업 흐름 정책, 리뷰 기록)와 `just linux-test`(제외한 묶음의 nextest, 제외 테스트 필터)를 돈다. 테스트는 네트워크 이름공간 안에서 돈다 | 제외 목록 밖 crate가 Linux에서 빌드되지 않으면 clippy·nextest가 실패한다. Linux에는 nextest wrapper(`sandbox-exec`)가 없어서 이름공간으로 막는다(ADR 0013의 넘김) | `just check` 전체(Linux에서 빌드 불가), `--network none` 컨테이너(도구를 다시 설치해야 한다), `unshare -rn`(Ubuntu 24.04의 AppArmor가 권한 없는 사용자 이름공간을 막는다) | harness 1.2, 6.2 |
| 보호된 Linux 제외 목록 `harness/ci/linux/exclusions.json`: crate `src-tauri`(wry가 webkit2gtk·glib를 요구, 실측 실패), `e2e-app`(`src-tauri`에 의존하는 E2E 앱), `platform-macos`(LocalAuthentication), `secrets-keychain`(데이터 보호 키체인). 테스트 모듈 `xtask::network_sandbox_tests`, `xtask::nested_sandbox_tests`(`sandbox-exec`). `exclusions.mjs verify`가 `just check`에서 이름이 작업 공간에 있는지, 제외된 crate에 의존하는 crate가 모두 제외됐는지, 모듈 선언이 있는지 본다 | 1.2 "macOS 전용 crate와 그것에 의존하는 crate". `harness/`라 보호 경로다 | 실패하는 crate만 제외(지금 비어 있는 macOS 어댑터가 나중에 macOS API를 쓰는 순간 Linux 작업이 깨진다) | harness 1.2, 7.1 |
| macOS 작업은 깨끗한 checkout에서 `lefthook install` 뒤 `just base=<기준> check`(밤은 `base=HEAD`) | `hooks-installed`가 `just check`의 마지막 단계다(ADR 0015) | `hooks-installed`를 CI에서 빼기 | ADR 0015 |
| 기준 SHA(`harness/ci/ci-range.mjs`): `master` 푸시는 `github.event.before`. 그것이 0(삭제 뒤 다시 만들기·첫 푸시)이거나 가져올 수 없으면(강제 푸시로 사라짐) 실패한다. 강제 푸시면 기준은 그대로 두고 알린다(macOS의 `test-diff`는 조상이 아니라서 실패한다). 다른 브랜치 푸시는 `origin/master` 끝(새 브랜치 포함). PR은 대상 브랜치 끝(`pull_request.base.sha`)과 PR 머리 | `before..after`는 병합 커밋과 fast-forward로 들어온 커밋을 모두 포함한다. 첫 푸시는 이미 지났으므로 0은 삭제 뒤 다시 만든 경우다 | 브랜치 이름 `merge-base`(ADR 0011·0013) | ADR 0013, 0015 |
| 실패 알림: 모든 실패는 작업 실패(푸시한 사람에게 GitHub 메일, 커밋의 빨간 표시). `master` 푸시와 밤 작업의 실패는 `notify` 작업이 이슈 "CI 실패: master"/"CI 실패: 밤 작업"을 열거나(열린 것이 있으면 댓글) 남긴다. `notify`만 `issues: write`를 갖는다 | 브랜치 실패는 메인이 푸시 직후 본다. `master`와 밤 실패는 다음 브리프 전에 메인이 확인해야 하므로(workflow 2.1) 남는 기록이 필요하다 | 모든 실패에 이슈(브랜치 실험마다 이슈가 쌓인다), 메일만 | workflow 2.1 |
| 컨테이너 시각 회귀 단계는 두지 않는다. DS-15가 기준 환경과 엔진을 정하면 그 작업이 Linux arm64 작업에 단계를 더한다 | 기반이 없는 단계는 가짜 통과다 | 자리만 두기 | harness 6.6, 브리프 |

### 캐시

| 결정 | 근거 | 버린 대안 |
|---|---|---|
| 캐시는 `~/.cargo/registry/index`, `~/.cargo/registry/cache`, `~/.cargo/git/db`(키: OS·아키텍처·`Cargo.lock` 해시)와 `cargo install`한 cargo-mutants 실행 파일(키: OS·아키텍처·`harness/ci/tools.json`·`rust-toolchain.toml` 해시)뿐이다 | 레지스트리의 `.crate` 파일은 풀 때 `Cargo.lock`의 체크섬과 비교된다. 판정에 영향을 주는 산출물이 아니다 | - |
| `target/`, `node_modules/`, `~/.cargo/registry/src`(이미 풀린 소스, 다시 검사하지 않는다), 권고 DB는 캐시하지 않는다. 권고 DB는 매번 `just advisory-db-fetch`로 받는다 | ADR 0013·0015: 낡은 빌드와 바꿔치기한 산출물이 결과를 바꿀 수 있다. 풀린 소스 바꿔치기는 ADR 0015의 알려진 빈틈이다 | `Swatinem/rust-cache`(`target/` 캐시), `setup-node`의 pnpm 캐시 |
| pnpm 저장소도 캐시하지 않는다(설치는 잠금 파일 무결성 해시로 검증되고 몇 초다) | 단순함 | `actions/cache`로 pnpm store |

### 도구 고정과 권한

| 결정 | 근거 | 버린 대안 |
|---|---|---|
| Rust는 `rust-toolchain.toml`(러너의 rustup이 읽는다), Node는 `.node-version`(`setup-node`의 `node-version-file`), pnpm은 `packageManager`(corepack, sha512 포함). `just`·`cargo-nextest`·`cargo-deny`·`cargo-llvm-cov`는 `taiki-e/install-action`(내려받은 파일을 매니페스트의 sha256과 비교)에 정확한 버전으로, `cargo-mutants`는 `cargo install --locked --version 27.1.0`(arm64 바이너리가 없다), lefthook은 `harness/ci/tools.mjs`가 릴리스 파일을 받아 `tools.json`의 sha256과 비교. 버전 목록은 `harness/ci/tools.json` 한 곳이고, 테스트가 lefthook 고정(`harness/git-hooks/lefthook.toml`)과 이 PC의 실제 버전이 같은지 본다 | harness 1.3. just는 `justfile.json`(just의 해석 결과)을 비교하므로 버전이 바뀌면 결과가 달라질 수 있다 | `cargo install` 전부(느리다), 최신 버전 |
| 서드파티 액션은 커밋 SHA로만 쓰고 정책 파일의 SHA와 같아야 한다. 최상위 `permissions`는 `contents: read`만, 작업별 권한은 정책에 적은 `notify`의 `issues: write`만 | 태그는 옮길 수 있다. 쓰기 권한은 필요한 작업에만 | - |
| 포크 PR: `pull_request`만 쓰고 `pull_request_target`·`workflow_run`은 금지, `secrets.` 참조 금지, checkout은 `persist-credentials: false`, `run`에 `${{ }}` 금지(값은 `env`로). 재사용 작업 흐름·로컬 액션 금지, `continue-on-error` 금지, 러너는 목록 셋만 | 포크 PR은 읽기 토큰만 받고 비밀이 없다. `run` 안의 식은 커밋 메시지·PR 제목으로 셸 주입이 된다. `continue-on-error`는 가짜 통과다 | - |
| 정책 검사 `harness/ci/workflow-policy/check.mjs`(`just check`와 Linux 작업): `.github/` 아래에 정책의 파일 목록 밖 파일이 있거나 목록 파일이 없으면, 또는 위 규칙을 어기면 실패. 예제 21개(`cases.json`, 예제는 Prettier가 건드리지 않게 `.workflow` 확장자), 정책 원소 하나를 뺀 변형마다 저장소나 예제가 실패하는지 본다 | harness 6.8 | actionlint(Go 바이너리 의존성 추가) |
| `.github/**`는 허용 목록 밖이라 보호 경로다. 잠금 조각은 추적 파일에서 `./.github`를 만들므로 `policy.json`의 새 경로 패턴에서 `.github`를 뺐다(중복이면 원소 빼기 테스트가 실패한다). 설정 파일 이름 검사에 GitHub가 읽는 `action.yml`, `action.yaml`, `dependabot.yml`, `dependabot.yaml`, `CODEOWNERS`를 더했다(허용 위치 없음, 편집 예제 5개) | 작업 흐름은 `.github/workflows`만 읽히고 그 폴더는 정책 검사가 맡는다. 다른 이름은 어디에 있든 GitHub 동작을 바꿀 수 있다 | - |

### 보호 경로 서명 검사 (H-26)

| 결정 | 근거 | 버린 대안 |
|---|---|---|
| `signatures` 작업은 기준 커밋을 별도 작업 사본으로 꺼내(`harness/ci/signatures/from-base.mjs`) 그 사본의 `harness/ci/signatures/verify.mjs`를 돌린다. 신뢰 자료(`harness/allowed_signers`, `harness/agent-writable.txt`, `harness/ci/signatures/lock-point.txt`), 허용 목록 판정 코드(`.claude/hooks/lib/allowlist.mjs`), xtask(`test-changes`, `manifest-changes`)를 모두 그 사본에서 쓴다. cargo는 그 사본을 현재 폴더로 돌아 푸시된 쪽의 `.cargo/config.toml`·`rust-toolchain.toml`을 읽지 않는다. git은 전역·시스템 설정, 대체 객체, graft 없이 부른다. 기준의 검증 스크립트가 요약 줄(`서명 검사: 위반 N건`) 없이 끝나면 실패한다(임시 폴더가 심볼릭 링크 아래면 스크립트의 진입점 판정이 어긋나 아무것도 하지 않고 0으로 끝났다. 실측) | 검사 대상 커밋이 검증 수단을 바꿀 수 없다(harness 7.6) | 푸시된 쪽 스크립트(R-10 3.4 초안을 그대로 돌리면 같은 푸시에서 바꿀 수 있다) |
| 대상은 `rev-list <대상> ^<기준> ^<잠금 시점>`의 커밋 하나하나(병합으로 들어온 쪽 커밋 포함). 부모 대비 바뀐 경로 중 (a) 허용 목록 밖, (b) 허용 목록 안의 `Cargo.toml`에서 `[dependencies]`·`[dev-dependencies]`·`[build-dependencies]`(밑줄 별칭 포함)·`[workspace.dependencies]` 밖이 바뀐 것(xtask가 `toml`로 읽어 비교, 읽지 못하면 바뀐 것으로 본다), (c) 허용 목록 안의 `package.json`에서 `dependencies`·`devDependencies`·`optionalDependencies`·`peerDependencies` 밖이 바뀐 것, (d) 부모에 있던 `*.test.mjs`의 변경·삭제, (e) `.rs`가 바뀐 커밋에서 `xtask test-changes <기준> <부모> <커밋>`이 보고한 기존 테스트 변경(기준 커밋에 있던 테스트·테스트 모듈만. 같은 브랜치에서 새로 더한 테스트를 뒤 커밋이 고치는 것은 통과)이 하나라도 있으면 그 커밋은 기준의 `allowed_signers`로 `git verify-commit`이 통과해야 한다 | 커밋마다 보므로 되돌린 변경도 잡는다. pre-commit 훅을 건너뛴 커밋도 같다. Rust TOML은 여러 줄 문자열에 표 머리줄을 숨길 수 있어 줄 단위로 나누면 속는다(예제) | 범위 전체 차이(되돌리면 사라진다), 줄 단위 TOML 분할 |
| 병합 커밋은 부모마다 위 이유를 구하고 모든 부모에 공통인 이유만 서명을 요구한다(어느 부모와도 다른 내용, 즉 충돌 해결과 덧붙인 변경). 한쪽 부모에서 온 내용은 그 쪽 커밋이 범위 안이면 따로 검사되고, 기준이나 잠금 시점 이전에서 왔으면 이미 받아들인 것이다 | `git diff-tree --cc`와 같은 기준 | 첫 부모 대비(병합한 브랜치 전체가 병합 커밋의 변경으로 보인다) |
| 잠금 시점: `harness/ci/signatures/lock-point.txt`(지금 비어 있음)에 40자리 SHA 한 줄. 비어 있으면 "서명 검사 비활성: 잠금 시점 없음"을 출력하고 통과한다. 단 범위 안에서 잠금 시점을 기록한 커밋은 그 커밋 자신의 `allowed_signers`에 있는 키로 서명했고 기록한 SHA가 그 커밋의 조상이어야 한다(아니면 실패). 기록된 뒤에는 그 SHA의 조상은 보지 않는다. 기준 커밋에 검증 스크립트가 없으면(이 작업이 들어가기 전의 기준) "서명 검사 비활성: 기준 커밋 …에 verify.mjs가 없다"를 출력하고 통과한다 | 지금까지의 이력은 서명 없이 보호 경로를 만들었다. 기록하는 커밋 자체는 아직 기준에 서명자가 없으므로 그 커밋의 목록으로 확인한다(처음 한 번의 신뢰). 그 뒤의 서명자 변경은 보호 경로 변경이라 기준의 목록으로 검사된다 | 처음부터 전체 이력 검사(지금 이력이 모두 실패한다) |
| 검증 로직은 `harness/ci/tests/signatures.test.mjs`(임시 저장소, 임시 ed25519 키)가 고정한다: 비활성, 서명 없음, 허용되지 않은 키, 빈 서명자 목록, 같은 푸시에서 서명자·허용 목록을 넓히고 자기 키로 서명, 서명 뒤 트리 변조, 되돌리기, 병합(양쪽 부모와 다른 내용만, 서명 병합이 데려온 서명 없는 커밋), fast-forward 범위, 의존성 절만(Cargo·package.json)과 그 밖, 기존 Rust 테스트·`.test.mjs` 변경과 새 테스트, 잠금 시점 이전, 잠금 시점 기록 커밋 | 브리프 8 | - |

### 리뷰 기록 검사 (H-27)

| 결정 | 근거 | 버린 대안 |
|---|---|---|
| `harness/ci/review-records/check.mjs`(`just check`와 Linux 작업): `docs/progress.md`에서 머리줄이 `작업 | 상태 | 대상 커밋 | 리뷰 기록`인 표의 행 중 상태가 `완료`·`일부 완료`(뒤에 괄호 설명 허용)인 행마다 리뷰 기록 칸이 `docs/reviews/*.md` 링크이고 파일이 있으며, 제목(`# `)에 행의 작업 ID(`H-40(Rust)` 같은 꼬리 포함)가 모두 있고, `## 커밋` 절에 `| 회차 | 기준 SHA | 대상 SHA |` 표(행 1개 이상), `## 리뷰…` 절에 `| 번호 | 등급 |` 표(행 1개 이상) 또는 내용이 있는 `## 리뷰 생략` 절, `## 측정` 절에 `| 항목 | 값 |` 표가 있어야 한다 | 여러 작업을 한 기록에 묶는다(`H-02, H-03` → `H-02.md`). 지금 저장소의 기록 17개가 모두 통과한다 | tasks.md의 상태(상태 칸이 없다), 파일 이름 = 작업 ID |
| 예제 14개는 `cases.json`에 글로 두고 테스트가 임시 폴더에 펼친다 | Prettier가 `harness/`의 `.md` 표를 다시 정렬한다 | `.md` 예제 파일 |

### 규칙 세트

| 결정 | 근거 | 버린 대안 |
|---|---|---|
| `master`(기본 브랜치)에 브랜치 규칙 세트 하나: 삭제 금지, 강제 푸시 금지. 우회 주체 없음 | 기준 SHA가 사라지거나 0이 되는 길(강제 푸시, 삭제 뒤 다시 만들기)을 원격에서 닫는다. 메인의 절차(병합은 `--no-ff`, 강제 푸시는 셸 훅이 거부)와 부딪히지 않는다 | - |
| "상태 검사 필수"(이 검사가 통과해야 `master`에 직접 푸시)는 쓰지 않는다 | (1) 작업 흐름 정의는 푸시된 커밋의 것이 돈다. 같은 푸시에서 `.github/workflows`를 바꿔 같은 이름의 작업이 항상 통과하게 하면 필수 검사도 통과한다. 원격 강제가 되지 않는다. (2) 직접 푸시하려면 병합 커밋을 다른 브랜치에 먼저 올리고 Linux 작업이 끝날 때까지 기다려야 한다(절차가 늘어난다, P-1). (3) 잠금 시점이 없는 동안 서명 검사는 비활성이라 막을 것이 없다 | 상태 검사 필수, "PR 필수"(사람이 병합, R-10 Q9), "서명 커밋 필수"(에이전트 커밋마다 Touch ID, ADR 0003) |
| 파일 경로 제한(푸시 규칙 세트)은 쓸 수 없다 | 공개 저장소에는 없다(비공개·내부 저장소 전용) | - |

사용자가 저장소 관리 권한으로 한 번 실행할 명령(메인 에이전트는 실행하지 않는다):

```sh
gh api -X POST repos/gihwan-dev/stock-copilot/rulesets --input - <<'JSON'
{"name":"master 보호","target":"branch","enforcement":"active","bypass_actors":[],"conditions":{"ref_name":{"include":["~DEFAULT_BRANCH"],"exclude":[]}},"rules":[{"type":"deletion"},{"type":"non_fast_forward"}]}
JSON
gh api repos/gihwan-dev/stock-copilot/rulesets
```

확인: 마지막 명령의 출력에 `"enforcement": "active"`인 규칙 세트가 하나 있다. 에이전트 세션의 `gh` 토큰에 관리 권한이 있으면 규칙 세트를 지울 수 있다. 에이전트가 쓰는 토큰은 저장소 관리 권한이 없는 세분화 토큰(Contents 읽기·쓰기, Issues 읽기)으로 바꾸는 것을 권한다(harness 7.6 "에이전트에게 주는 토큰에는 저장소 관리 권한을 주지 않는다").

## H-20 절차에 넣을 단계 (H-28 뒤)

ADR 0018의 H-20 절차 끝(서명 커밋 직전)에 더한다. 사용자가 Claude Code 밖에서:

1. H-28: `harness/allowed_signers`에 `<이메일> namespaces="git" <Secretive 공개키>` 한 줄. 저장소에 `git config gpg.format ssh`, `git config user.signingkey <공개키 파일>`, `git config commit.gpgsign false`.
2. 잠금 시점으로 쓸 커밋을 정한다: 검토를 마친 `master` 끝(보통 H-20 서명 커밋의 부모). `git rev-parse <커밋> > harness/ci/signatures/lock-point.txt`(40자리 + 줄바꿈).
3. `just base=<master> check` 뒤 `git commit -S`(이 커밋이 서명자 목록과 잠금 시점을 함께 기록한다). `git verify-commit HEAD`가 `Good "git" signature`를 내는지 확인한다.
4. 푸시 뒤 `signatures` 작업 출력에 `잠금 시점 기록: <커밋> → <SHA>`와 위반 0건이 나와야 한다. 다음 푸시부터 출력이 `서명 검사: 기준 …, 잠금 시점 …`으로 바뀐다.

## 메인이 반영할 문서 변경

- harness.md 1.2 표: "CI 푸시마다 | Linux arm64"의 내용을 "보호 경로 서명 검사(7.6), 리뷰 기록 검사, 작업 흐름 정책, 플랫폼 중립 crate 묶음의 형식·린트·deny·xtask 검사·테스트(네트워크 이름공간 안)"로, 컨테이너 시각 회귀는 "DS-15에서 더한다"로. "CI `main` 푸시와 밤"의 `main`을 `master`로, 지금 macOS 작업이 실제로 하는 것은 `lefthook install` 뒤 전체 `just check`뿐이고 경로 고정 테스트·앱 빌드·E2E·스크린샷은 해당 작업(U-03 등)에서 더한다고. "CI 밤 | Linux"는 "Linux 전체 묶음, `just mutants-all`, 권고 DB를 받은 뒤 `deny`"(오래 걸리는 속성 테스트와 AI 고정 자료 CLI 버전 점검은 해당 작업에서).
- harness.md 1.3 표에 행 추가: just 1.59.0, cargo-nextest 0.9.151, cargo-deny 0.20.2, cargo-llvm-cov 0.9.1, cargo-mutants 27.1.0, lefthook 2.2.2, pnpm 12.18.0(`packageManager`) — 목록은 `harness/ci/tools.json`. Node 행의 "고정 방법은 H-01에서 정한다"를 "`.node-version`"으로.
- harness.md 1.1 `just check` 행의 "CI 푸시마다"를 "CI `master` 푸시와 밤(macOS)"으로. Linux 푸시는 `just linux-check`·`just linux-test`.
- harness.md 7.6 "원격 강제 수단" 행: "공개 저장소. 규칙 세트는 `master` 삭제·강제 푸시 금지만 쓴다(ADR 0020). 상태 검사 필수·PR 필수·서명 필수는 쓰지 않는다". "검증" 행에 "잠금 시점(`harness/ci/signatures/lock-point.txt`) 이후 커밋만". "남는 빈틈"의 첫 항목(의존성 절 검사)은 해결됨. 서명자 목록 위치는 `harness/allowed_signers`.
- harness.md 7.1 "목록" 행의 "Linux CI 제외 목록"에 위치 `harness/ci/linux/exclusions.json`.
- workflow.md 2.1: "메인 에이전트는 새 작업의 브리프를 쓰기 전에 `gh run list --branch master --limit 3`와 열린 'CI 실패' 이슈가 없는지 확인한다". 3장의 "CI가 tasks.md에서 완료로 표시된 작업마다"를 "`docs/progress.md`에서 상태가 완료·일부 완료인 행마다"로.
- tasks.md H-27의 "tasks.md에서 `완료`인 작업"을 "progress.md의 완료 행"으로.
- open-issues S-21(비공개 저장소 규칙 세트)은 공개 저장소로 해소됐다고 적는다.

## 이 검사들이 잡지 못하는 것

(ADR 0019의 `just known-gaps` 개수를 바꾸지 않으려고 "남는 빈틈" 머리말을 쓰지 않았다. 목록에 넣으려면 메인이 ADR 0019의 개수 줄과 함께 고친다.)

- 작업 흐름 정의는 푸시된 커밋의 것이 돈다. 같은 푸시에서 `.github/workflows`를 바꿔 `signatures` 작업을 없애거나 항상 통과하게 하면 그 실행과 그 뒤 실행이 검사하지 않는다. 막는 것은 로컬 겹(편집 훅·셸 훅·잠금 조각의 `./.github`)이고, CI 쪽에는 "출력에 `서명 검사` 줄이 없다"는 사람의 확인뿐이다. 잠금 뒤에는 `.github` 변경이 보호 경로라 다음 정상 실행이 그 커밋을 실패로 보고하지만, 정의를 바꾼 뒤에는 정상 실행이 오지 않는다.
- 범위 밖 커밋: 잠금 시점 이전 커밋, 그리고 "기준"으로 쓰는 이전 `master` 끝 자체. 규칙 세트(강제 푸시·삭제 금지)가 없으면 이전 끝을 바꿔 기준을 옮길 수 있다.
- `xtask test-changes`는 `test-diff`의 정적 비교만 쓴다(테스트 목록·개수 비교 없음). `test-diff`가 원리상 놓치는 것(ADR 0013)과 TypeScript Vitest 테스트(`*.test.ts`) 변경(H-29 TS)은 잡지 못한다.
- 서명 검사는 서명이 있는지만 본다. 사용자가 무엇에 서명했는지는 사용자 몫이다.
- 리뷰 기록 검사는 절과 표가 있는지만 본다. 내용이 맞는지, harness 2.1의 "끄기 목록"이 있는지는 보지 않는다(지금 기록 대부분에 끄기 목록 절이 없다).
- Linux 네트워크 이름공간은 `sudo`가 필요하다. GitHub 러너에서는 암호 없는 `sudo`가 있지만 로컬 재현은 특권 컨테이너에서만 했다.
- 밤 macOS의 "그날 변경"은 `master` 끝 커밋의 커미터 시각으로 판단한다. 오래된 커밋을 그대로 fast-forward로 올리면 그날 밤 macOS가 돌지 않는다(다음 `master` 푸시의 macOS 작업이 돈다).
- `ci.yml`의 `notify`는 `master` 푸시 실패에만, `nightly.yml`의 `notify`는 밤 작업 실패에만 이슈를 연다. 브랜치 푸시 실패는 작업 실패와 메일뿐이다.

## 결과

- 새 just 레시피: `linux-check`, `linux-test`. `check-ts` 끝에 `workflow-policy`, `review-records`, `linux/exclusions.mjs verify`, `node --test 'harness/ci/tests/*.test.mjs'`. 하네스 검사 목록(`steps.json`)과 `justfile.json`에 등록했다.
- 새 xtask 명령: `test-changes <저장소> <기준> <부모> <커밋>`, `manifest-changes <저장소> <부모> <커밋> <경로>`(서명 검사가 기준 커밋의 xtask로 부른다).
- 커밋 훅의 check가 191초였다(기준 커밋 실측 약 3분 30초의 `just check`와 비슷하다. 새 node 테스트 묶음은 약 8초).
- 첫 푸시(이 브랜치)의 서명 작업은 기준(`origin/master`)에 검증 스크립트가 없어 비활성으로 통과한다. 이 작업이 `master`에 들어간 뒤부터 기준의 검증 스크립트가 돈다. 잠금 시점이 기록될 때까지는 "잠금 시점 없음"으로 통과한다.
- 바뀌면 다시 볼 조건: 저장소를 비공개로 돌릴 때(규칙 세트·러너 비용), GitHub가 공개 저장소에 푸시 규칙 세트나 "기본 브랜치의 정의로 도는 필수 작업 흐름"을 열 때(위 첫 빈틈을 닫을 수 있다), cargo-mutants가 arm64 바이너리를 낼 때, DS-15(시각 회귀), U-03·E2E 작업(macOS 작업 내용), 러너 이미지가 `sudo`·`unshare`·`setpriv`를 바꿀 때.
