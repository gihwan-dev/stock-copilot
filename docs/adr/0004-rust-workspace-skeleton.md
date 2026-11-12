# ADR 0004: Rust 작업 공간 뼈대와 Tauri CLI의 `target/` (실측 S-12)

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-01
- 반영한 문서: 없음 (architecture.md 3.1의 배치를 그대로 따른다)

## 맥락

architecture.md 3.1은 저장소 루트를 가상 작업 공간으로 두고 `target/`을 하나만 쓴다. Tauri CLI(`tauri build`, `tauri dev`)가 루트 가상 작업 공간의 `target/`을 제대로 다루는지는 미확인이었다(R-05 Q2, open-issues 실측 S-12). 다루지 못하면 배치를 조정해야 했다.

## 실측

환경: macOS 26 (Apple Silicon), `rust-toolchain.toml`의 Rust 1.99.1, `tauri` `=2.12.5`(`default-features = false`, `wry`), `tauri-build` `=2.7.4`, Tauri CLI `@tauri-apps/cli` 2.12.4(npm 최신 안정, `npx`로 실행. 저장소에 설치하지 않았다). 명령은 `apps/desktop`에서 돌렸다.

| 항목 | 명령 | 결과 |
|---|---|---|
| 릴리스 빌드 | `tauri build --no-bundle` | `Built application at: <저장소>/target/release/src-tauri`. 처음부터 33.74초 |
| 디버그 빌드와 앱 묶음 | `tauri build --debug --bundles app --config '{"bundle":{"active":true,"icon":["icons/icon.png"]}}'` | `<저장소>/target/debug/src-tauri`, 묶음은 `<저장소>/target/debug/bundle/macos/Stock Copilot.app` |
| 개발 실행 | `tauri dev --no-watch` | `cargo run --no-default-features`로 `<저장소>/target/debug/src-tauri`를 띄웠다 |
| `src-tauri/target` 생성 여부 | 위 세 명령 뒤 `ls apps/desktop/src-tauri` | 만들어지지 않았다 |
| `cargo metadata`의 `target_directory` | 테스트 `build_output_goes_to_root_target_directory` | 저장소 루트의 `target` |

CLI가 낸 정보 메시지: "`tauri` dependency has workspace inheritance enabled. The features array won't be automatically rewritten." CLI가 `tauri.conf.json`에 맞춰 `tauri`의 기능 목록을 고쳐 쓰지 않는다는 뜻이다. 기능 목록은 루트 `Cargo.toml`의 `[workspace.dependencies]`에서 손으로 관리한다.

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| architecture.md 3.1의 배치를 그대로 쓴다. 루트 가상 작업 공간, `target/` 하나 | Tauri CLI 2.12.4가 루트 `target/`으로 빌드·묶음·개발 실행을 모두 했다 | `src-tauri`를 작업 공간 루트로(R-05 (다)), `CARGO_TARGET_DIR` 고정 | 실측 S-12 |
| `src-tauri`의 패키지 이름은 `src-tauri`, lib 이름은 `src_tauri`. 기본 bin(`src/main.rs`)과 `gen-bindings`(`src/bin/gen-bindings.rs`) 두 개. `default-run = "src-tauri"` | architecture.md 3.2의 의존 표가 `src-tauri`라는 이름을 쓴다. bin이 둘이면 `cargo run`과 Tauri CLI가 실행할 bin을 알아야 한다 | 패키지 이름 `stock-copilot` | architecture.md 3.1, 3.2 |
| `build_app`은 지금 `(Builder<R>, Context<R>) -> Result<App<R>, tauri::Error>`다. `Ports` 인자는 포트가 생기는 작업에서 더한다 | 아직 포트가 없다 | 빈 `Ports` 타입을 미리 만들기 | architecture.md 3.1 |
| 빈 앱의 화면은 `src-tauri/empty-frontend/index.html`, 아이콘은 `src-tauri/icons/icon.png`(32×32 단색) | `generate_context!`가 컴파일 때 `frontendDist` 폴더와 `icons/icon.png`를 읽는다. 없으면 컴파일이 실패했다 | 루트의 `apps/desktop/dist`(H-21 전에는 없어서 깨끗한 사본의 `just check`가 실패) | 실측 |
| `tauri.conf.json`에 `app.security.capabilities: []`, `bundle.active: false` | 빈 앱에는 capability 파일이 없다. 빈 목록은 Tauri의 기본값과 같아서 `./capabilities/`의 파일을 모두 포함한다(tauri-utils `config.rs`). 즉 명시적 목록이 아니다. 묶음은 서명 작업(S-14)에서 켠다 | - | R-05 Q1 |

## 결과

- H-21이 프런트엔드를 만들면 `frontendDist`를 그 산출 폴더로 바꾸고 `empty-frontend/`를 지운다. 아이콘은 디자인 산출물이 오면 바꾼다.
- 앱 묶음의 `Contents/MacOS`에 `src-tauri`와 함께 `gen-bindings`가 들어갔다. Tauri 묶음은 패키지의 bin을 모두 담는다. 제품 묶음에 바인딩 생성기가 들어가지 않게 하는 방법은 H-13 또는 S-14에서 정한다.
- 명시적 capability 목록(`["main"]`)은 U-03에서 넣는다. H-33의 `capability-check`는 빈 목록을 명시로 보지 않아야 한다.
- `tauri dev`는 `--locked` 없이 `cargo run`을 부른다. 검사는 `just check`만 믿는다.
- Tauri CLI 버전은 아직 저장소에 고정하지 않았다. H-21의 pnpm 작업 공간에서 `@tauri-apps/cli`를 정확한 버전으로 고정한다.
- 바뀌면 다시 볼 조건: Tauri CLI를 3.x로 올릴 때, `tauri build`가 `src-tauri/target`을 만들기 시작할 때.
