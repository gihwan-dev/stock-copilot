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
| 레지스트리 캐시 무결성(수정 1차) | 임시 `CARGO_HOME`에서 `itoa 1.0.15`를 받은 뒤 `registry/cache`의 `.crate`를 `compile_error!`를 더한 내용으로 바꾸고 `registry/src`를 지운 뒤 `cargo build` | 바뀐 소스가 그대로 컴파일돼 `tampered` 오류. 체크섬 다시 확인 없음 |
| 첫 CI 실행(0d77ed9, `ci`) | GitHub Actions | `signatures` 9초(비활성 출력), `linux` 4분 42초: cargo-mutants 컴파일 49초, `just linux-check` 통과, 이름공간 안 nextest 209개 통과 |
| 리뷰 기록의 커밋 존재(수정 1차) | 원격 저장소를 새로 클론해 `review-records/check.mjs` | 기록 9개(H-05, H-07, H-08, H-09, H-11, H-21, H-30, H-31, H-39)의 리베이스 전 커밋이 원격에 없어 20건 실패. 로컬(원본 저장소)에서는 0건 |

로컬 테스트(수정 1차 뒤): `node --test 'harness/ci/tests/*.test.mjs'`(서명 검사 23개는 임시 저장소와 임시 SSH 키를 쓴다), xtask 새 테스트 10개.

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
| 밤 Linux의 "AI 고정 자료의 CLI 버전 점검"(harness 1.2)은 지금 넣지 않는다. 대상이 되는 녹화 스트림 고정 자료(architecture.md 2장, AI-6의 CLI 버전 기록)와 그 CLI 버전 표시가 저장소에 아직 없다(`fixtures/`·`prompts/` 없음). AI 어댑터 작업(R-02·R-03의 녹화 스트림을 고정 자료로 넣는 작업, AI-16)이 고정 자료에 CLI 버전을 기록하면, 그 작업이 밤 Linux에 "고정 자료의 CLI 버전과 `npm view @anthropic-ai/claude-code version`·`npm view @openai/codex version`(로그인 없이 되는 버전 조회) 비교, 다르면 이슈" 단계를 더한다 | 비교할 대상이 없는 단계는 가짜 통과다 | 지금 빈 비교 단계 |

### 캐시

| 결정 | 근거 | 버린 대안 |
|---|---|---|
| 캐시를 쓰지 않는다. 매 실행 `cargo fetch --locked`(Cargo.lock 체크섬 비교), `pnpm install --frozen-lockfile`(무결성 해시), `cargo install --locked cargo-mutants`(컴파일, CI 실측 49초), 권고 DB 받기를 새로 한다 | 실측(아래 "수정 1차 실측"): cargo는 `registry/cache`에 이미 있는 `.crate` 파일을 체크섬과 다시 비교하지 않는다. 내용을 바꾼 `.crate`가 그대로 빌드됐다. 캐시를 쓰는 작업은 같은 브랜치·기본 브랜치 범위의 캐시를 쓰므로 바꿔치기한 캐시가 판정을 바꿀 수 있다 | 레지스트리 캐시(첫 판, 위 주장이 틀렸다), cargo-mutants 실행 파일 캐시(첫 판, 무결성 확인 없음), `Swatinem/rust-cache` |

### 도구 고정과 권한

| 결정 | 근거 | 버린 대안 |
|---|---|---|
| Rust는 `rust-toolchain.toml`(러너의 rustup이 읽고 구성 요소의 sha256을 확인한다), Node는 `.node-version`(`setup-node`의 `node-version-file`), pnpm은 `packageManager`(corepack, sha512 포함). `just`·`cargo-nextest`·`cargo-deny`·`cargo-llvm-cov`는 `taiki-e/install-action`(내려받은 파일을 매니페스트의 sha256과 비교)에 정확한 버전으로. `cargo-mutants` 27.1.0은 arm64 Linux·arm64 macOS 배포 파일이 없어(x86_64만) 매번 `cargo install --locked --version 27.1.0`으로 컴파일한다(crates.io 체크섬 확인, 캐시 없음). lefthook은 `harness/ci/tools.mjs`가 릴리스 파일을 받아 `tools.json`의 sha256과 비교. 체크섬 확인 없는 내려받기는 `setup-node`의 Node 내려받기 하나다(공식 액션, SHA 고정). 버전 목록은 `harness/ci/tools.json` 한 곳이고, 테스트가 lefthook 고정(`harness/git-hooks/lefthook.toml`)과 이 PC의 실제 버전이 같은지 본다 | harness 1.3 | 공식 x86_64 실행 파일을 x86_64 러너에서(Linux arm64 배치와 어긋난다), 캐시 |
| 서드파티 액션은 커밋 SHA로만 쓰고 정책 파일의 SHA와 같아야 한다. 최상위 `permissions`는 `contents: read`만, 작업별 권한은 정책에 적은 `notify`의 `issues: write`만 | 태그는 옮길 수 있다. 쓰기 권한은 필요한 작업에만 | - |
| 포크 PR: `pull_request`만 쓴다. 비밀 값 문맥(`secrets`라는 낱말 어디든)은 실패, checkout은 `persist-credentials: false` | 포크 PR은 읽기 토큰만 받는다 | `pull_request_target` |
| 정책 검사 `harness/ci/workflow-policy/check.mjs`(`just check`와 Linux 작업)는 허용 목록 방식이다(수정 1차). 최상위 키 `name`·`on`·`permissions`·`jobs`만, 이벤트 `push`·`pull_request`(옵션은 `branches`만)·`schedule`(정책의 cron만), 작업 키 `runs-on`·`timeout-minutes`(필수, 1~180)·`steps`·`needs`(같은 파일의 작업)·`if`·`permissions`·`outputs`만, 단계 키 `name`(필수)·`id`·`uses`·`with`·`run`·`env`·`if`만이고 `uses`와 `run` 중 하나. `run`은 정책의 명령 목록(16개)과 글자까지 같아야 하고, `if`는 조건 목록(6개)과, `${{ }}` 값은 식 목록과, `env` 이름은 이름 목록과, 액션은 하위 경로까지 포함한 이름과 SHA가 목록과, `with` 키는 액션별 목록과 값은 키별 허용 값과 같아야 한다. `.github/` 아래 파일은 정책의 두 파일만. 예제 47개(`cases.json`, 리뷰어가 통과시킨 `paths-ignore`·`paths`·`branches-ignore`·`concurrency`·`if`·`container`·`services`·`env`의 `secrets.`·`secrets[`·`toJSON(secrets)`·`github.event` 식·`curl | sh`·`actions/cache/save@…`·`defaults`·`continue-on-error`·`timeout-minutes` 누락·`runs-on`·`secrets: inherit`·재사용 작업 흐름 포함)와 정책 원소 하나를 뺀 변형마다 저장소나 예제가 실패하는지 본다 | 금지 목록(첫 판)은 새 키·새 식으로 계속 샌다 | 금지 목록, actionlint |
| `.github/**`는 허용 목록 밖이라 보호 경로다. 잠금 조각은 추적 파일에서 `./.github`를 만들므로 `policy.json`의 새 경로 패턴에서 `.github`를 뺐다(중복이면 원소 빼기 테스트가 실패한다). 설정 파일 이름 검사에 GitHub가 읽는 `action.yml`, `action.yaml`, `dependabot.yml`, `dependabot.yaml`, `CODEOWNERS`를 더했다(허용 위치 없음, 편집 예제 5개) | 작업 흐름은 `.github/workflows`만 읽히고 그 폴더는 정책 검사가 맡는다. 다른 이름은 어디에 있든 GitHub 동작을 바꿀 수 있다 | - |

### 보호 경로 서명 검사 (H-26)

| 결정 | 근거 | 버린 대안 |
|---|---|---|
| `signatures` 작업은 기준(`master` 푸시는 `github.event.before`, 다른 브랜치는 `master`, PR은 대상 브랜치 SHA)을 두 번째 checkout(`base-tools`)으로 받고, 거기 있는 `harness/ci/signatures/from-base.mjs`를 실행한다(수정 1차). `from-base.mjs`는 자기 checkout의 HEAD를 기준 SHA로 삼아 작업 공간 밖 임시 폴더에 기준 작업 사본을 만들고 그 사본의 `verify.mjs`, 신뢰 자료(`harness/allowed_signers`, `harness/agent-writable.txt`, `harness/ci/signatures/lock-point.txt`), 허용 목록 코드, xtask를 쓴다. cargo는 그 사본을 현재 폴더로 돌아 푸시된 쪽의 `.cargo/config.toml`·`rust-toolchain.toml`을 읽지 않는다(작업 공간 안의 `base-tools`에서 돌리면 상위 폴더의 설정을 읽는다). git은 전역·시스템 설정, 대체 객체, graft 없이 부른다. 기준에 `from-base.mjs`가 없으면 "서명 검사 비활성: 기준 checkout에 … 없다"를 출력한다. 검증 스크립트가 요약 줄 없이 끝나면 실패한다. 푸시된 쪽에서 도는 것은 작업 흐름 파일 자체뿐이다(남는 빈틈) | 검사 대상 커밋이 검증 로직과 신뢰 자료를 바꿀 수 없다 | 푸시된 쪽의 `from-base.mjs`·`ci-range.mjs`(첫 판) |
| 대상은 `rev-list <대상> ^<기준> ^<잠금 시점>`의 커밋 하나하나(병합으로 들어온 쪽 커밋 포함). 부모 대비 바뀐 경로 중 (a) 허용 목록 밖, (b) 허용 목록 안의 `Cargo.toml`에서 의존성 표(`[dependencies]`·`[dev-dependencies]`·`[build-dependencies]`·밑줄 별칭·`[workspace.dependencies]`) 밖이 바뀌었거나, 의존성 표가 바뀌었는데 레지스트리 버전 문자열·`version`·`features`·`default-features`·`optional`·`package`·`workspace = true`·저장소 안을 가리키는 `path` 밖의 항목(`git`, `registry`, 저장소 밖 `path`, URL)이 있는 것(xtask가 `toml`로 읽어 비교, 읽지 못하면 바뀐 것), (c) 허용 목록 안의 `package.json`에서 `dependencies`·`devDependencies`·`optionalDependencies`·`peerDependencies` 밖이 바뀌었거나 의존성 값이 `[~^]?x.y.z[-pre]`·`npm:<이름>@` 그 형식이 아닌 것(`file:`, `link:`, `git+`, URL, `workspace:`, `*`, `latest`), (d) 부모에 있던 `*.test.mjs`의 변경·삭제, (e) `.rs`가 바뀐 커밋에서 `xtask test-changes <기준> <부모> <커밋>`이 보고한 기존 테스트 변경(기준 커밋에 있던 테스트만)이 하나라도 있으면 그 커밋은 기준의 `allowed_signers`로 `git verify-commit`이 통과해야 한다. `[patch]`·`[replace]`·`[profile]`·`build`·`[workspace]`·`[target.*.dependencies]`, `pnpm.overrides`·`overrides`·`resolutions`·`scripts`·`packageManager`·`engines` 변경은 서명이 필요하다(테스트로 고정) | 커밋마다 보므로 되돌린 변경도 잡는다. 훅을 건너뛴 커밋도 같다. Rust TOML은 여러 줄 문자열에 표 머리줄을 숨길 수 있어 줄 단위로 나누면 속는다(예제) | 범위 전체 차이, 줄 단위 TOML 분할, 의존성 값 내용 무시(첫 판) |
| 병합 커밋(문어발 포함): 어느 부모에 대해서든 생기는 이유(경로·테스트) 하나하나마다, 병합 결과가 그 이유에 대해 같은 부모 Pj가 있고, 결과가 다른 나머지 부모 Pi마다 "Pi와 Pj의 병합 기준 대비 Pj 쪽이 바꾼 것"이면 설명된 것으로 본다. 설명되지 않는 이유가 하나라도 있으면 서명을 요구한다(수정 1차) | `git merge`가 만드는 결과(한쪽이 바꾼 내용을 가져옴)만 서명 없이 통과한다. 조상인 부모(병합 기준 = 그 부모)는 아무것도 설명하지 못하므로 `commit-tree <옛 보호 파일 트리> -p c2 -p c0`(부모 순서 무관, 문어발 포함)도, 옛 커밋 위에서 자란 곁가지의 옛 내용도 서명이 필요하다. 지금 `master`를 브랜치로 병합하는 정상 경우는 `master` 쪽이 바꾼 내용이라 통과한다 | 부모별 이유의 교집합(첫 판, 조상 부모로 되돌리기가 통과했다), 첫 부모 대비 |
| 잠금 시점: `harness/ci/signatures/lock-point.txt`(지금 비어 있음)에 40자리 SHA 한 줄. 비어 있으면 "서명 검사 비활성: 잠금 시점 없음"을 출력하고 통과한다. 범위 안에서 처음 잠금 시점을 기록한 커밋은 (1) 그 커밋 자신의 `allowed_signers`(형식 검사 포함)에 있는 키로, (2) 저장소 소유자의 GitHub 계정에 등록된 SSH 서명 키(`GET /users/<GITHUB_REPOSITORY_OWNER>/ssh_signing_keys`, 공개 API, `from-base.mjs`가 받아 `--owner-keys` 파일로 넘긴다)로 서명했고, (3) 기록한 SHA가 그 커밋의 조상이어야 한다. 소유자 키 대조는 CI만 한다(로컬에는 목록이 없어 "판정할 수 없다"로 실패). 기록된 뒤에는 그 SHA의 조상은 보지 않는다. 기록된 잠금 시점을 비우거나 파일을 지우는 커밋은 서명이 있어도 실패, 다른 값으로 바꾸는 커밋은 새 값이 기존 잠금 시점의 자손이고 그 커밋의 조상이어야 하며 서명이 필요하다(보호 경로) | 에이전트가 키를 만들어 자기 목록에 넣고 먼저 기록하는 길을 막는다. 잠금을 비우면 검사 전체가 꺼진다 | 그 커밋의 목록만 신뢰(첫 판) |
| `allowed_signers`(기준 것과 기록 커밋의 것)는 `<이름> namespaces="git" <ed25519|ecdsa-nistp256|sk 계열> <키> [설명 한 낱말]` 줄만 받는다. `valid-before`·`valid-after`·`cert-authority` 같은 옵션, 다른 namespace, 주석 줄, ssh-rsa는 실패. 키 폐기는 목록에서 지우는 것으로만 한다. git은 서명 키로 principal을 찾으므로 principal 이름은 커미터와 묶이지 않는다(서명한 키가 목록에 있는지만 본다) | 커미터 시각을 앞당기면 `valid-before`로 만료시킨 키가 통과했다(리뷰 재현). namespace `git` 밖 키는 `verify-commit`이 받지 않는다(테스트) | 만료 옵션으로 폐기 |
| 검증 로직은 `harness/ci/tests/signatures.test.mjs`(임시 저장소, 임시 ed25519 키, 23개)가 고정한다: 비활성, 서명 없음, 허용되지 않은 키, 빈 서명자 목록, 같은 푸시에서 서명자·허용 목록을 넓히고 자기 키로 서명, 서명 뒤 트리 변조, 되돌리기, 병합(양쪽과 다른 내용, 서명 병합이 데려온 서명 없는 커밋, 조상 부모·순서 바꿈·문어발, 옛 곁가지, `master`를 브랜치로 병합), fast-forward 범위, 의존성 절만과 그 밖, 기존 Rust 테스트·`.test.mjs`, 잠금 시점 이전, 잠금 시점 기록(소유자 키 일치·불일치·목록 없음), 잠금 시점 옮기기·비우기·지우기, 서명자 목록 형식. 핵심 조건을 하나씩 뒤집으면(서명 확인 항상 참, 잠금 변경 무시, `.test.mjs` 무시, 병합 설명의 "Pj 쪽이 바꿨나"·"Pj와 같은가") 각각 1~17개 테스트가 실패한다 | 브리프 8 | - |

### 리뷰 기록 검사 (H-27)

| 결정 | 근거 | 버린 대안 |
|---|---|---|
| `harness/ci/review-records/check.mjs`(`just check`와 Linux 작업, 수정 1차): `docs/progress.md`의 모든 표를 읽는다. 첫 칸이 작업 ID 목록(`H-02, H-03`, `H-40(Rust)` 꼴)인 행은 `작업 | 상태 | 대상 커밋 | 리뷰 기록` 머리줄의 표에만 있어야 한다. 그 표의 상태 칸은 `완료`, `일부 완료`, `완료 (N회차까지)`, `해결` 중 하나와 글자까지 같아야 하고(`**완료**`, `완료됨`은 실패), `해결` 밖의 세 값이면 리뷰 기록 칸이 `docs/reviews/*.md` 링크이고 파일이 있으며, 제목(`# `)에 행의 작업 ID가 모두 있고, `## 커밋` 절의 `| 회차 | 기준 SHA | 대상 SHA |` 표에 행이 있고 행마다 회차가 비어 있지 않고 두 SHA 칸이 7~40자리 16진수로 시작하며 그 커밋이 저장소에 있고, `## 리뷰…` 절의 `| 번호 | 등급 |` 표에 행이 있고 행마다 번호·등급이 있거나 `## 리뷰 생략` 절에 `- 결정: 사용자|메인 에이전트` 줄과 20자 이상의 `- 사유: …` 줄이 있고, `## 측정` 절의 `| 항목 | 값 |` 표에 행이 있고 행마다 두 칸이 차 있어야 한다 | 여러 작업을 한 기록에 묶는다. 리뷰어가 빈 칸·다른 표·꾸민 상태로 통과시켰다 | tasks.md의 상태, 첫 행만 보기(첫 판) |
| 예제 29개는 `cases.json`에 글로 두고(`{commit}` 자리에 임시 저장소의 커밋), 테스트가 임시 저장소에 펼친다. 표 열·빈 칸 열·커밋 열·절을 하나씩 빼면 어떤 예제가 실패하는지 본다 | Prettier가 `harness/`의 `.md` 표를 다시 정렬한다 | `.md` 예제 파일 |

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

1. H-28: `harness/allowed_signers`에 `<이메일> namespaces="git" <Secretive 공개키>` 한 줄. 저장소에 `git config gpg.format ssh`, `git config user.signingkey <공개키 파일>`, `git config commit.gpgsign false`. 같은 공개키를 저장소 소유자 GitHub 계정에 서명 키로 등록한다(웹 Settings > SSH and GPG keys > New SSH key, 종류 Signing Key). `curl -s https://api.github.com/users/gihwan-dev/ssh_signing_keys`에 그 키만 보이는지 확인한다.
2. 잠금 시점으로 쓸 커밋을 정한다: 검토를 마친 `master` 끝(보통 H-20 서명 커밋의 부모). `git rev-parse <커밋> > harness/ci/signatures/lock-point.txt`(40자리 + 줄바꿈).
3. `just base=<master> check` 뒤 `git commit -S`(이 커밋이 서명자 목록과 잠금 시점을 함께 기록한다). `git verify-commit HEAD`가 `Good "git" signature`를 내는지 확인한다.
4. 잠금 시점을 기록한 뒤 `harness/ci/signatures/lock-point.txt`는 비울 수 없다(서명이 있어도 실패). 옮길 때는 기존 잠금 시점의 자손인 커밋으로만, 서명 커밋으로.
5. 푸시 뒤 `signatures` 작업 출력에 `잠금 시점 기록: <커밋> → <SHA>`와 위반 0건이 나와야 한다. 다음 푸시부터 출력이 `서명 검사: 기준 …, 잠금 시점 …`으로 바뀐다.

## 메인이 반영할 문서 변경

- harness.md 1.2 표: "CI 푸시마다 | Linux arm64"의 내용을 "보호 경로 서명 검사(7.6), 리뷰 기록 검사, 작업 흐름 정책, 플랫폼 중립 crate 묶음의 형식·린트·deny·xtask 검사·테스트(네트워크 이름공간 안)"로, 컨테이너 시각 회귀는 "DS-15에서 더한다"로. "CI `main` 푸시와 밤"의 `main`을 `master`로, 지금 macOS 작업이 실제로 하는 것은 `lefthook install` 뒤 전체 `just check`뿐이고 경로 고정 테스트·앱 빌드·E2E·스크린샷은 해당 작업(U-03 등)에서 더한다고. "CI 밤 | Linux"는 "Linux 전체 묶음, `just mutants-all`, 권고 DB를 받은 뒤 `deny`"(오래 걸리는 속성 테스트와 AI 고정 자료 CLI 버전 점검은 해당 작업에서).
- harness.md 1.3 표에 행 추가: just 1.59.0, cargo-nextest 0.9.151, cargo-deny 0.20.2, cargo-llvm-cov 0.9.1, cargo-mutants 27.1.0, lefthook 2.2.2, pnpm 12.18.0(`packageManager`) — 목록은 `harness/ci/tools.json`. Node 행의 "고정 방법은 H-01에서 정한다"를 "`.node-version`"으로.
- harness.md 1.1 `just check` 행의 "CI 푸시마다"를 "CI `master` 푸시와 밤(macOS)"으로. Linux 푸시는 `just linux-check`·`just linux-test`.
- harness.md 7.6 "원격 강제 수단" 행: "공개 저장소. 규칙 세트는 `master` 삭제·강제 푸시 금지만 쓴다(ADR 0020). 상태 검사 필수·PR 필수·서명 필수는 쓰지 않는다". "검증" 행에 "잠금 시점(`harness/ci/signatures/lock-point.txt`) 이후 커밋만". "남는 빈틈"의 첫 항목(의존성 절 검사)은 해결됨. 서명자 목록 위치는 `harness/allowed_signers`.
- harness.md 7.1 "목록" 행의 "Linux CI 제외 목록"에 위치 `harness/ci/linux/exclusions.json`.
- workflow.md 2.1: "메인 에이전트는 새 작업의 브리프를 쓰기 전에 `gh run list --branch master --limit 3`와 열린 'CI 실패' 이슈가 없는지 확인한다". 3장의 "CI가 tasks.md에서 완료로 표시된 작업마다"를 "`docs/progress.md`에서 상태가 완료·일부 완료인 행마다"로.
- tasks.md H-27의 "tasks.md에서 `완료`인 작업"을 "progress.md의 완료 행"으로.
- open-issues S-21(비공개 저장소 규칙 세트)은 공개 저장소로 해소됐다고 적는다.

## 남는 빈틈

- 작업 흐름 파일(`.github/workflows/*.yml`)은 푸시된 커밋의 것이 돈다. 기준 checkout의 `from-base.mjs`를 부르는 단계 자체를 같은 푸시에서 지우거나 `echo`로 바꾸면 그 실행과 그 뒤 실행이 검사하지 않는다. 막는 것은 로컬 겹(편집 훅·셸 훅·잠금 조각의 `./.github`)과 `just check`의 작업 흐름 정책(정책 파일도 보호 경로라 함께 바꿔야 한다)이고, CI 쪽에는 "출력에 `서명 검사:` 요약 줄이 없다"는 사람의 확인뿐이다. 공개 저장소에서는 기본 브랜치의 정의로 도는 필수 작업 흐름을 쓸 수 없다.
- 강제 푸시 금지 규칙 세트가 없으면 `master`를 기준과 무관한 이력으로 바꿔 올릴 수 있다. 병합 판정은 "기준이 대상의 조상"인 이력을 전제한다.
- 처음 잠금 시점을 기록할 때의 소유자 키 대조는 GitHub 계정에 등록된 서명 키를 믿는다. 사용자의 `gh` 토큰에 `admin:ssh_signing_key` 권한이 있고 에이전트가 그 토큰을 쓸 수 있으면 에이전트가 자기 키를 계정에 등록할 수 있다. 에이전트용 토큰에서 그 권한을 빼고, 계정의 서명 키 목록에 Secretive 키 하나만 있는지 사용자가 확인한다.
- 리뷰 기록의 커밋 표는 저장소에 있는 커밋만 받는다. 리베이스 전 커밋처럼 푸시하지 않은 커밋은 새 클론에 없어 CI에서 실패한다.
- 범위 밖 커밋: 잠금 시점 이전 커밋, 그리고 "기준"으로 쓰는 이전 `master` 끝 자체. 규칙 세트(강제 푸시·삭제 금지)가 없으면 이전 끝을 바꿔 기준을 옮길 수 있다.
- `xtask test-changes`는 `test-diff`의 정적 비교만 쓴다(테스트 목록·개수 비교 없음). `test-diff`가 원리상 놓치는 것(ADR 0013)과 TypeScript Vitest 테스트(`*.test.ts`) 변경(H-29 TS)은 잡지 못한다.
- 서명 검사는 서명이 있는지만 본다. 사용자가 무엇에 서명했는지는 사용자 몫이다.
- 리뷰 기록 검사는 절·표·칸의 형식과 커밋 존재만 본다. 내용이 맞는지, harness 2.1의 "끄기 목록"이 있는지는 보지 않는다(지금 기록 대부분에 끄기 목록 절이 없다).
- Linux 네트워크 이름공간은 `sudo`가 필요하다. GitHub 러너에서는 암호 없는 `sudo`가 있지만 로컬 재현은 특권 컨테이너에서만 했다.
- 밤 macOS의 "그날 변경"은 `master` 끝 커밋의 커미터 시각으로 판단한다. 오래된 커밋을 그대로 fast-forward로 올리면 그날 밤 macOS가 돌지 않는다(다음 `master` 푸시의 macOS 작업이 돈다).
- `ci.yml`의 `notify`는 `master` 푸시 실패에만, `nightly.yml`의 `notify`는 밤 작업 실패에만 이슈를 연다. 브랜치 푸시 실패는 작업 실패와 메일뿐이다.

## 결과

- 새 just 레시피: `linux-check`, `linux-test`. `check-ts` 끝에 `workflow-policy`, `review-records`, `linux/exclusions.mjs verify`, `node --test 'harness/ci/tests/*.test.mjs'`. 하네스 검사 목록(`steps.json`)과 `justfile.json`에 등록했다.
- 새 xtask 명령: `test-changes <저장소> <기준> <부모> <커밋>`, `manifest-changes <저장소> <부모> <커밋> <경로>`(서명 검사가 기준 커밋의 xtask로 부른다).
- 새 레시피·스크립트(수정 1차): `harness/ci/notify.mjs`(이슈 열기·댓글), `tools.mjs fetch`(cargo fetch, 미리 받기, pnpm 설치), `tools.mjs lefthook`(설치와 `GITHUB_PATH`).
- 커밋 훅의 check가 191초였다(기준 커밋 실측 약 3분 30초의 `just check`와 비슷하다. 새 node 테스트 묶음은 약 8초).
- 첫 푸시(이 브랜치)의 서명 작업은 기준(`origin/master`)에 검증 스크립트가 없어 비활성으로 통과한다. 이 작업이 `master`에 들어간 뒤부터 기준의 검증 스크립트가 돈다. 잠금 시점이 기록될 때까지는 "잠금 시점 없음"으로 통과한다.
- 바뀌면 다시 볼 조건: 저장소를 비공개로 돌릴 때(규칙 세트·러너 비용), GitHub가 공개 저장소에 푸시 규칙 세트나 "기본 브랜치의 정의로 도는 필수 작업 흐름"을 열 때(위 첫 빈틈을 닫을 수 있다), cargo-mutants가 arm64 바이너리를 낼 때(sha256 고정 내려받기로 바꾼다), GitHub이 공개 저장소에 필수 작업 흐름을 열 때, DS-15(시각 회귀), U-03·E2E 작업(macOS 작업 내용), 러너 이미지가 `sudo`·`unshare`·`setpriv`를 바꿀 때.
