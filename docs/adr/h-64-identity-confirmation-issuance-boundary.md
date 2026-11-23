# ADR H-64: 사용자 종목 식별 확인 값의 발급 경계

- 상태: 제안 (번호는 메인 병합 시 부여)
- 날짜: 2026-11-24
- 결정한 사람: 구현 담당 초안(사용자·메인 확인 대기)
- 관련 작업: H-64 (연관 D-55, S-24, A-02, A-04, H-04, H-20)
- 반영한 문서: `crates/domain/src/instrument_mapping_change.rs`, `crates/application/src/identity_confirmation_prompt.rs`, `crates/application/src/confirm_instrument_identity.rs`, `crates/testkit-ports/src/scripted_identity_confirmation.rs`, `crates/testkit-ports/src/memory.rs`, `crates/xtask/src/arch_check/**`, `harness/arch-check/rules.toml`, `harness/arch-check/cases.toml`, `harness/agent-writable.txt`, `harness/claude-hooks/hashes.mjs`, `harness/claude-hooks/edit-cases.json`

## 맥락

D-55([ADR 0157](0157-instrument-identity-history.md))는 확인된 종목 식별자 변경(`link_confirmed_change`), 폐지 정정(`correct_retirement`), 연결 정정(`correct_link`)이 `IdentityConfirmationOrigin::UserConfirmed(UserIdentityConfirmation)`을 소비하도록 봉인했다. 그러나 `UserIdentityConfirmation`의 발급 경로가 아직 없어 `application`·`testkit-ports`·`store-sqlite` 어느 크레이트도 세 쓰기 연산의 성공 경로를 실행·검증할 수 없고, S-24(이력·정정 영속)의 쓰기 단계가 이에 막혀 있다.

`UserPresence` 선례([ADR 0007](0007-arch-check-graph-and-source-location.md), [ADR 0080](0080-storage-and-system-ports.md), [ADR 0091](0091-testkit-fakes.md))를 따라 다음 제약을 동시에 만족하는 발급 경계를 설계한다.

- 발급의 근거는 "사용자가 확인 화면에서 직접 확인했다"는 사실뿐이다. 그 사실을 운반하는 포트를 `application`에 두고, 확인 유스케이스는 포트의 `Confirmed` 결과에서만 확인 값을 만든다. 확인 화면과 플랫폼 어댑터 구현은 이번 작업 범위 밖이다.
- `domain`의 발급 함수는 한 곳(`UserIdentityConfirmation::issue_after_user_confirmation`)에만 두고, `#[cfg(test)]` 우회나 feature 게이트를 쓰지 않는다.
- `xtask arch-check`가 발급 함수 호출 위치와 확인 포트 구현·별칭·재수출 위치를 기계적으로 고정한다.
- `UserIdentityConfirmation`과 이를 감싸는 요청 타입(`ConfirmedIdentifierChange`, `ConfirmedRetirementCorrection`, `ConfirmedLinkCorrection`, `IdentityConfirmationOutcome`)의 일회성(`Clone`·`Copy`·`Default` 부재, 소비 후 재사용 컴파일 실패)과 기존 D-55 컴파일 실패 프로브 14건을 그대로 유지한다.
- 시험용 발급 가짜(`crates/testkit-ports/src/scripted_identity_confirmation.rs`)는 `harness/agent-writable.txt`·샌드박스·Codex 설정·Claude 훅 해시에 보호 등록한다.
- `testkit-ports`의 메모리 가짜(`MemoryInstrumentMappingRepository`)에서 세 쓰기 연산의 성공 경로를 새 발급 경계로 실행·검증해 S-24 2단계가 즉시 사용할 수 있음을 보인다.

## 발급 경계의 구조

| 주체 / 위치 | `UserIdentityConfirmation` 발급 | `IdentityConfirmationPrompt` 구현 | 근거·강제 수단 |
|---|---|---|---|
| `crates/domain/src/instrument_mapping_change.rs` | 유일한 정의(`UserIdentityConfirmation::issue_after_user_confirmation`) | 불가 | `arch-check` `identity-confirmation-issuance`가 단일 `const fn` 시그니처·비공개 봉인 필드·허용 derive(`Debug`/`PartialEq`/`Eq`)·보조 생성자/trait impl/형제 모듈 부재를 검사 |
| `crates/domain/src/instrument_mapping_change/history_cases.rs` | 내부 단위 테스트 전용(`#[cfg(test)] mod tests`, 비공개 항목만) | 불가 | `defining-test-file` 규칙으로 공개 항목·`issue_after_user_confirmation` 호출 금지 유지 |
| `crates/application/src/confirm_instrument_identity.rs` | 가능(`ConfirmInstrumentIdentity`의 `Prompt: IdentityConfirmationPrompt` 구속 `async fn` 안에서 `match self.prompt.confirm_*().await`의 `Confirmed` 분기당 1회 직접 호출만) | 불가 | `issuance-allowed-files` + `use-case-file` AST 검사: 비공개 `prompt` 필드, 원천 모듈(`crate::identity_confirmation_prompt`) 구속, 루프·클로저·이중 호출·래퍼·원시 확인 값 반환·비공개/공개 재수출 금지 |
| `crates/testkit-ports/src/scripted_identity_confirmation.rs` | 가능(`ScriptedIdentityConfirmationIssuer::issue`의 `IdentityConfirmationDecision::Confirmed` 분기만, 항상 성공 생성자 없음) | 가능(`ScriptedIdentityConfirmation`) | `issuance-allowed-files` + `prompt-impl-files` + `agent-writable.txt` 보호 등록 + 샌드박스 `denyWrite` + Codex `read` + Claude 해시 검증 + `testkit-ports` 제품 바이너리(`src-tauri`) 및 `domain`/`application` 의존 금지 |
| `crates/adapters/platform-macos` | 불가 | 가능(향후 확인 화면/OS 확인 구현 위치) | `prompt-impl-crates = ["platform-macos"]`; 발급 함수 호출은 `identity-confirmation-issuance`로 거부 |
| `store-sqlite`, `broker-toss`, `broker-toss-trade`, `sources`, `ai-claude-code`, `ai-codex`, `secrets-keychain`, `mcp-server`, `scheduler`, `src-tauri`, `order-execution`, `approval-command`, `trade-fakes`, `domain` 타 파일, `application` 타 파일, `testkit-ports` 타 파일 | 불가(직접·별칭·raw ident `r#`·함수 포인터·제네릭 콜백·매크로 토큰·Cargo 의존 이름 바꾸기 모두 거부) | 불가(직접·raw ident·매크로 impl, `as` 별칭, `pub use`/글롭/모듈 별칭/Cargo 이름 바꾸기 재수출 모두 거부) | `arch-check` 4규칙 + Rust 비공개 봉인 필드 + `source-check`(금지 매크로·`#[path]`·`#[allow]`) |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| `domain::UserIdentityConfirmation::issue_after_user_confirmation() -> Self` 단일 `const fn`만 공개하고, `UserIdentityConfirmationSeal`은 비공개 유지하며 `Clone`·`Copy`·`Default`를 부여하지 않는다 | 의도를 이름으로 드러내고 `#[cfg(test)]` 우회나 Cargo feature 게이트 없이(`source-check` 준수) 소비 후 재사용과 복제를 컴파일 단계에서 계속 차단한다. D-55의 기존 컴파일 실패 프로브 14건을 한 줄도 바꾸지 않고 유지한다 | `#[cfg(feature = "test-issuance")]` 게이트(금지); `From<...>`/`Default` 구현(위조·재사용 위험); `domain` 내부에서 포트 trait 직접 참조(의존 방향 위반) | H-64 메인 결정 2·3, ADR 0157 |
| 확인 포트 `application::identity_confirmation_prompt::IdentityConfirmationPrompt`는 `UserPresence` 선례([ADR 0080](0080-storage-and-system-ports.md))와 동일하게 루트 재내보내기하지 않고 전용 모듈에만 둔다. 입력 타입(`IdentifierChangePrompt`, `RetirementCorrectionPrompt`, `LinkCorrectionPrompt`)과 닫힌 판정(`IdentityConfirmationDecision::{Confirmed, Denied, Unavailable}`)은 루트에서 재내보낸다 | `UserPresence`와 같은 포트 가시성·별칭/재수출 차단 모델(`identity-confirmation-prompt-impl`/`alias`/`reexport`)을 적용해 어댑터가 확인 포트를 임의 구현하거나 다른 이름으로 위장하지 못하게 한다 | `application` 루트에 `IdentityConfirmationPrompt` 재내보내기; `UserPresence`에 메서드 추가(A-02 고정 계약 테스트 `ports_a02_contract.rs` 충돌 및 주문 본인 확인과 종목 식별 확인의 책임 혼합) | H-64 메인 결정 1, ADR 0080 |
| 확인 유스케이스 `ConfirmInstrumentIdentity<'a, Prompt: IdentityConfirmationPrompt, Time: Clock>`는 세 연산(`confirm_identifier_change`, `confirm_retirement_correction`, `confirm_link_correction`)마다 `self.prompt.confirm_*(&request).await`가 `Confirmed`일 때만 `self.clock.now()`와 `UserIdentityConfirmation::issue_after_user_confirmation()`을 호출해 즉시 도메인 요청 타입(`ConfirmedIdentifierChange`·`ConfirmedRetirementCorrection`·`ConfirmedLinkCorrection`)으로 감싸 `IdentityConfirmationOutcome<T>`로 반환한다 | 원시 `UserIdentityConfirmation`을 호출자에게 노출하지 않고(서명 누출 금지), `Denied`·`Unavailable`에서는 확인 값 발급과 시계 조회가 모두 일어나지 않는다. `IdentityConfirmationOutcome<T>`도 `Clone`·`Copy`·`Default`가 없어 일회성을 유지한다 | 유스케이스가 `UserIdentityConfirmation`을 직접 반환(다른 요청에 전용 가능); 동기 반환으로 `Prompt`를 호출하지 않고 확인 값 생성(위조 경로) | H-64 메인 결정 1·2·3 |
| `harness/arch-check/rules.toml`에 `[source.identity-confirmation]` 표를 두고 4개 규칙(`identity-confirmation-issuance`, `identity-confirmation-prompt-impl`, `identity-confirmation-prompt-alias`, `identity-confirmation-prompt-reexport`)을 `xtask arch-check`로 강제한다 | 단순 위치 검사만으로는 허용 파일 내부 변조(보조 생성자, `Clone` 추가, 래퍼 함수, 루프 다중 발급, 다른 모듈의 동명 trait 임포트, 하위 파일 `sneak.rs`)를 놓친다. 정의 파일·유스케이스 파일·보호 가짜 파일·비허용 파일 각각에 맞춘 AST+토큰 검사와 7개 회귀 예제로 우회 경로를 닫는다 | 단순 문자열 검색(변수명 오탐 또는 `r#`·별칭 우회); 허용 파일 내부 무검사(래퍼 함수로 경계 무력화) | H-64 메인 결정 2·3·4, ADR 0007 |
| 보호 시험 가짜 `crates/testkit-ports/src/scripted_identity_confirmation.rs`를 `agent-writable.txt`, `edit-cases.json`, `hashes.mjs`에 등록하고 공식 생성기로 `lock-fragment.json`·`.codex/config.toml`·`hashes.json`을 갱신한다 | `scripted_presence.rs`([ADR 0091](0091-testkit-fakes.md))와 같은 보호 계층을 적용한다. `ScriptedIdentityConfirmation`은 요청 종류별 독립 FIFO 큐 3개와 호출 기록을 제공하고 항상 성공 생성자가 없다. `ScriptedIdentityConfirmationIssuer`도 대본 FIFO(`Confirmed` 분기만 발급, 소진 시 패닉)로만 동작한다 | 보호 미등록 가짜; 항상 `Confirmed` 반환하는 편의 생성자(거짓 통과 유발) | H-64 작업 행, ADR 0091 |
| `InstrumentMappingRepository` 포트는 확장하지 않고, `MemoryInstrumentMappingRepository`에 고유 메서드(`link_confirmed_change`, `correct_retirement`, `correct_link`, `history`, `history_for_id`)만 추가해 발급 경계 위 세 쓰기 성공 경로를 검증한다 | 영속 포트 확장 형태(기존 저장소 확장 대 별도 정정 포트)는 D-55 정의 대기 9번과 S-24의 결정 범위이므로 H-64에서 미리 고정하지 않는다 | H-64에서 `InstrumentMappingRepository` trait에 메서드 임의 추가(S-21 구현 `store-sqlite`와 충돌 및 S-24 결정 선점) | ADR 0157 정의 대기 9, ADR 0158 |

## arch-check 규칙과 예제

### 규칙 (`harness/arch-check/rules.toml` `[source.identity-confirmation]`)

| 규칙 이름 | 검사 내용 |
|---|---|
| `identity-confirmation-issuance` | `issue_after_user_confirmation`과 `UserIdentityConfirmationSeal`의 사용을 정의 파일(`instrument_mapping_change.rs`), 허용 파일(`confirm_instrument_identity.rs`, `scripted_identity_confirmation.rs`)로 제한. 비허용 크레이트·파일의 모든 토큰(직접·별칭·`r#`·함수 포인터·제네릭·매크로), `domain` 형제 파일의 `impl`/`type`/별칭, 정의 파일의 `Clone`/`Default`/공개 필드/보조 메서드/trait impl/하위 파일, 유스케이스 파일의 원시 확인 값 반환·래퍼·비구속 impl·비`Confirmed` 분기·루프·클로저·이중 호출·위조 trait 임포트·하위 파일, 보호 가짜의 상수 `Confirmed` 매치·재수출을 거부 |
| `identity-confirmation-prompt-impl` | `impl ... IdentityConfirmationPrompt for ...`(raw ident 및 매크로 포함)를 `prompt-impl-crates`(`platform-macos`)와 `prompt-impl-files`(`crates/testkit-ports/src/scripted_identity_confirmation.rs`)로 제한 |
| `identity-confirmation-prompt-alias` | `IdentityConfirmationPrompt`의 `as` 별칭(`use ... as`, `self as`, `r#`, 매크로 토큰 포함)을 전 크레이트에서 거부 |
| `identity-confirmation-prompt-reexport` | `IdentityConfirmationPrompt`, `identity_confirmation_prompt`(및 모듈 별칭), Cargo 의존 이름을 바꾼 `application` 별칭의 공개 재수출(`pub use`, `pub(...) use`, 글롭 재수출, 매크로 포함)을 거부 |

### 신규 예제 (`harness/arch-check/cases.toml`)

1. `identity-confirmation-allowed-pass` — 정의 파일, 내부 테스트 파일, `confirm_instrument_identity.rs`, `platform-macos` 포트 구현, `scripted_identity_confirmation.rs` 보호 가짜가 모두 위반 0건으로 통과하는 양성 대조.
2. `identity-confirmation-issuance-in-adapters-fail` — 11개 어댑터·호스트 크레이트(`store-sqlite`, `broker-toss`, `broker-toss-trade`, `sources`, `ai-claude-code`, `ai-codex`, `secrets-keychain`, `platform-macos`, `mcp-server`, `scheduler`, `src-tauri`)에서 발급 호출 거부.
3. `identity-confirmation-issuance-outside-allowed-files-fail` — 허용 크레이트 안의 비허용 파일(`application/src/sync_portfolio.rs`, `domain/src/instrument_mapping.rs`, `testkit-ports/src/lib.rs`, `testkit-ports/src/memory.rs`)에서 발급 호출 거부.
4. `identity-confirmation-indirect-calls-fail` — raw ident(`r#`), 제네릭 함수 포인터, 매크로(`vec![]`), 타입 별칭+UFCS, Cargo 의존 이름 바꾸기(`dom`)를 통한 간접 호출 거부.
5. `identity-confirmation-defining-and-use-case-tampering-fail` — 정의 파일의 보조 생성자·`Clone`·하위 파일 `sneak.rs`, 유스케이스 파일의 무검증 래퍼·하위 파일 `sneak.rs` 거부.
6. `identity-confirmation-prompt-impl-outside-allowed-fail` — 8개 비허용 크레이트·파일(`store-sqlite`, `broker-toss`, `ai-claude-code`, `sources` 매크로, `application`, `mcp-server`, `scheduler`, `testkit-ports/src/lib.rs`)에서 `IdentityConfirmationPrompt` 구현 거부.
7. `identity-confirmation-prompt-alias-and-reexport-fail` — `as` 별칭(`store-sqlite`), `r#`+`self as`(`mcp-server`), 직접 `pub use`(`broker-toss`), 모듈 별칭 글롭(`ai-claude-code`), bare 모듈 글롭(`scheduler`), 모듈 재수출(`sources`), Cargo 이름 바꾸기 글롭(`secrets-keychain`) 거부.

## 정의 대기

아래 정책은 기존 문서가 확정하지 않았으므로 임의로 정하지 않았다. 현재 구현은 가장 보수적인 기본값을 따른다.

| 항목 | 현재 안전한 기본값 | 선택지 | 사용자 영향 |
|---|---|---|---|
| 1. 실제 확인 화면 어댑터 위치 | `prompt-impl-crates = ["platform-macos"]`(`UserPresence` 선례) | A) `platform-macos`가 macOS 네이티브 확인 대화상자/인증으로 `IdentityConfirmationPrompt` 구현 / B) `src-tauri`의 앱 내 확인 화면 IPC 경계에서 `IdentityConfirmationPrompt` 구현(`rules.toml` 허용 확장 + 별도 IPC 위조 방어 필요) | A는 WebView 침해로부터 확인 사실을 격리하지만 네이티브 OS 대화상자가 뜬다. B는 앱 UI 흐름과 자연스럽게 이어지나 WebView-IPC 신뢰 경계 설계가 추가된다. |
| 2. 확인 화면의 본인 확인 강도 (P-9 단순함 원칙) | 포트 계약은 `Confirmed`/`Denied`/`Unavailable` 판정만 추상화(생체인증 필수 여부 미강제) | A) 종목 식별자 변경·정정은 주문이 아니므로 변경 전/후 심볼·ISIN·공시·사유를 표시하는 확인 화면의 명시적 확인 클릭만으로 발급 / B) 주문(`UserPresence`)처럼 macOS 생체인증(Touch ID/암호)까지 요구 | A는 P-9 단순함 원칙에 맞고 오기록 정정·티커 변경이 간결하다. B는 물리적 접근 방어가 강하지만 주문이 아닌 원장 정정마다 생체인증을 요구해 번거롭다. |
| 3. `confirm_instrument_identity.rs`·`instrument_mapping_change.rs`의 `agent-writable.txt` 보호 승격 시점 | 시험용 가짜 `scripted_identity_confirmation.rs`만 `agent-writable.txt`·샌드박스·Codex·Claude 해시에 보호 등록; 두 제품 파일은 `arch-check` AST/토큰 규칙으로 위조 경로 차단 | A) D-55 정의 대기·S-24 연동이 끝날 때까지 현재 유지 후 H-20 잠금 전에 보호 목록에 일괄 편입 / B) 두 제품 파일도 즉시 `agent-writable.txt` 보호에 추가 | A는 후속 정책 확정(D-55 정의 대기 2~11번, S-24 연동) 시 일반 작업 흐름을 막지 않으면서 위조는 `arch-check`로 차단한다. B는 에이전트 편집 자체를 즉시 막지만 정당한 정책 변경마다 사용자 서명 커밋이 필요하다. |
| 4. 확인 유스케이스와 영속 저장소의 결합 형태 (D-55 정의 대기 9번·S-24 연동) | `ConfirmInstrumentIdentity`는 확인 값(`IdentityConfirmationOutcome<Confirmed*>`)만 발급해 반환하고 저장소 포트에 직접 의존하지 않음 | A) S-24 포트 확정 뒤 확인+저장을 하나의 상위 유스케이스로 묶거나 `ConfirmInstrumentIdentity` 결과를 소비해 원자 저장 / B) 현재처럼 확인 발급 유스케이스를 분리 유지 | A는 확인 직후 저장 누락·중간 유실 가능성을 줄인다. B는 확인 흐름과 저장소 포트 사이의 결합을 낮춘다. |
| 5. 보호 가짜의 직접 발급기(`ScriptedIdentityConfirmationIssuer`) 유지 범위 | 보호 파일 `scripted_identity_confirmation.rs`에 포트 가짜(`ScriptedIdentityConfirmation`)와 대본 기반 직접 발급기(`ScriptedIdentityConfirmationIssuer`, `Denied`/`Unavailable`/`Confirmed` FIFO)를 함께 제공 | A) 둘 다 보호 파일에 유지해 S-24 저장소 단독 테스트가 결함 증거 조합(`UserConfirmed` + 불완전 증거 등)과 정상 쓰기를 모두 간결하게 시험 / B) 직접 발급기를 제거하고 모든 테스트가 `ConfirmInstrumentIdentity` + `ScriptedIdentityConfirmation`만 경유 | 제품 바이너리(`src-tauri`)는 `testkit-ports`에 의존할 수 없어 사용자 런타임 영향은 없다. A는 S-24 계약 테스트 작성이 간결하고, B는 테스트 코드에서도 유스케이스 경계를 강제한다. |
| 6. 확인 시각(`confirmed_at`/`corrected_at`) 기준과 확인 유효 시간(TTL) | `Confirmed` 직후 `Clock::now()`를 기록하고 별도 만료 시각은 두지 않으며 일회성 소유권 소비만 강제 | A) 확인 직후 시각 기록 + 메모리 일회성 소비 유지 / B) 확인 시각과 저장 적용 시각 사이 최대 허용 시간(TTL)을 두고 초과 시 거부 | A는 저장 지연·잠금 대기 시 불필요한 재확인이 없다. B는 오래된 확인의 지연 적용을 막으나 저장 지연 시 사용자가 다시 확인해야 한다. |

## S-24 2단계 인계 API

- **포트 (`application::identity_confirmation_prompt`)**:
  - `IdentityConfirmationPrompt::confirm_identifier_change(&self, &IdentifierChangePrompt) -> impl Future<Output = IdentityConfirmationDecision> + Send`
  - `IdentityConfirmationPrompt::confirm_retirement_correction(&self, &RetirementCorrectionPrompt) -> impl Future<Output = IdentityConfirmationDecision> + Send`
  - `IdentityConfirmationPrompt::confirm_link_correction(&self, &LinkCorrectionPrompt) -> impl Future<Output = IdentityConfirmationDecision> + Send`
- **확인 유스케이스 (`application`)**:
  - `ConfirmInstrumentIdentity::new(&prompt, &clock)`
  - `confirm_identifier_change(IdentifierChangePrompt) -> IdentityConfirmationOutcome<ConfirmedIdentifierChange>`
  - `confirm_retirement_correction(RetirementCorrectionPrompt) -> IdentityConfirmationOutcome<ConfirmedRetirementCorrection>`
  - `confirm_link_correction(LinkCorrectionPrompt) -> IdentityConfirmationOutcome<ConfirmedLinkCorrection>`
- **보호된 시험 가짜 (`testkit_ports`)**:
  - `ScriptedIdentityConfirmation::new(changes, retirements, links)` + `calls()` / `identifier_change_calls()` / `retirement_correction_calls()` / `link_correction_calls()`
  - `ScriptedIdentityConfirmationIssuer::new(decisions)` + `issue(IdentityConfirmationIssuanceKind) -> IdentityConfirmationOutcome<UserIdentityConfirmation>` + `calls()`
  - `MemoryInstrumentMappingRepository::link_confirmed_change` / `correct_retirement` / `correct_link` / `history` / `history_for_id`

## 실측 및 손 변형

### 커버리지·정적·변이 검사 (1차 구현)

- `just coverage-check`: 위반 없음 — `crates/domain/src/instrument_mapping_change.rs` 줄 232/232(100.00%)·영역 206/206(100.00%), `crates/application/src/identity_confirmation_prompt.rs` 줄 174/174(100.00%)·영역 236/236(100.00%).
- `cargo llvm-cov nextest --locked -p application -p testkit-ports -E 'binary(identity_confirmation_boundary) | test(identity_confirmation)' --summary-only`: `confirm_instrument_identity.rs` 줄 68/68(100.00%)·영역 79/79(100.00%)·함수 7/7(100.00%), `identity_confirmation_prompt.rs` 줄 174/174(100.00%)·영역 236/236(100.00%), `scripted_identity_confirmation.rs` 줄 87/87(100.00%)·영역 77/77(100.00%)·함수 15/15(100.00%).
- `just mutants-daily 4c13d306ed6190f2fbd970822a92ddf88f1a7529 0/1`: 위반 없음 — 대상 4개 파일(`confirm_instrument_identity.rs`, `identity_confirmation_prompt.rs`, `application/src/lib.rs`, `instrument_mapping_change.rs`)에서 선택된 27개 변이 전부 unviable(생존 0, 타임아웃 0; 봉인·프롬프트·결과 타입이 의도적으로 `Default`·`From`·무검증 생성자를 두지 않아 `Default::default()` 치환이 컴파일 단계에서 차단됨). 실제 논리 회귀는 아래 격리 복사본 손 변형 26건으로 보완했다.
- `arch_check_cases_tests::every_case_reports_exactly_its_findings` 및 `removing_any_rule_element_breaks_some_case`: 모두 통과(41.45초, 253.56초). `just test-diff 4c13d306ed6190f2fbd970822a92ddf88f1a7529` 위반 없음, `just disable-report-since 4c13d306ed6190f2fbd970822a92ddf88f1a7529` 새 끄기 0건.

### 격리 복사본 손 변형 26건 (26/26 검출, 생존 0, 컴파일 불가 0)

| 번호 | 대상 파일 | 무력화한 조건 | 검출 테스트 |
|---|---|---|---|
| M01 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `is_unauthorized_child_file`: 하위 파일 차단 비활성화 | `defining_file_children_and_test_module_are_closed`, `use_case_file_requires_prompt_gated_confirmed_arm_and_blocks_wrappers` |
| M02 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `check_unauthorized_file`: 비허용 파일의 `issuer_function` 토큰 검사 제거 | `unauthorized_internal_files_and_domain_siblings_are_rejected`, `unauthorized_adapters_and_indirect_calls_are_rejected` |
| M03 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `check_unauthorized_file`: 비허용 파일의 `seal_type` 토큰 검사 제거 | `unauthorized_adapters_and_indirect_calls_are_rejected` |
| M04 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `UnauthorizedFileCheck::visit_item_impl`: 타 파일 `impl UserIdentityConfirmation` 차단 제거 | `unauthorized_internal_files_and_domain_siblings_are_rejected`, `spoofed_prompt_imports_nested_structs_literal_matches_and_parens_are_rejected` |
| M05 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `type_last_name`: 괄호 타입(`Type::Paren`) 풀기 제거 | `spoofed_prompt_imports_nested_structs_literal_matches_and_parens_are_rejected` |
| M06 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `UnauthorizedFileCheck::visit_item_type`: `domain` 내 `type` 별칭 차단 제거 | `unauthorized_internal_files_and_domain_siblings_are_rejected` |
| M07 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `DefiningFileCheck::visit_item_struct`: `UserIdentityConfirmation`의 비허용 derive(`Clone` 등) 차단 제거 | `defining_file_accepts_canonical_issuer_and_rejects_backdoors` |
| M08 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `DefiningFileCheck::visit_item_impl`: 단일 메서드(`i.items.len()==1`) 검사 제거 | `defining_file_accepts_canonical_issuer_and_rejects_backdoors` |
| M09 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `DefiningFileCheck::visit_expr_path`: 정의 파일 내 비표준 경로의 봉인 생성 차단 제거 | `defining_file_accepts_canonical_issuer_and_rejects_backdoors` |
| M10 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `check_defining_test_file`: `history_cases.rs` 내 비공개 아닌 항목 차단 제거 | `test_module_impl_async_block_and_local_trait_are_rejected` |
| M11 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `UseCaseFileCheck::check_signature_leaks`: 유스케이스 시그니처의 원시 `UserIdentityConfirmation` 노출 차단 제거 | `test_module_impl_async_block_and_local_trait_are_rejected` |
| M12 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `UseCaseFileCheck::visit_item_impl`: `Prompt` 제네릭의 `IdentityConfirmationPrompt` 구속 검사 제거 | `spoofed_prompt_imports_nested_structs_literal_matches_and_parens_are_rejected`, `use_case_file_requires_prompt_gated_confirmed_arm_and_blocks_wrappers` |
| M13 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `UseCaseFileCheck::check_prompt_import_source`: 위조 모듈에서 `IdentityConfirmationPrompt` 임포트 차단 제거 | `spoofed_prompt_imports_nested_structs_literal_matches_and_parens_are_rejected` |
| M14 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `UseCaseFileCheck::visit_expr_call`: 유스케이스 `Confirmed` 분기 외부 발급 차단 제거 | `spoofed_prompt_imports_nested_structs_literal_matches_and_parens_are_rejected`, `use_case_file_requires_prompt_gated_confirmed_arm_and_blocks_wrappers` |
| M15 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `UseCaseFileCheck::visit_expr_call`: `Confirmed` 분기당 1회 호출 상한 검사 제거 | `use_case_file_requires_prompt_gated_confirmed_arm_and_blocks_wrappers` |
| M16 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `UseCaseFileCheck::visit_expr_call`: 루프·클로저·`async` 블록 내부 발급 차단 제거 | `use_case_file_requires_prompt_gated_confirmed_arm_and_blocks_wrappers`, `test_module_impl_async_block_and_local_trait_are_rejected` |
| M17 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `ProtectedFakeCheck::visit_expr_match`: 상수 `Confirmed` 매치 차단 제거 | `spoofed_prompt_imports_nested_structs_literal_matches_and_parens_are_rejected` |
| M18 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `PromptCheck::visit_item_impl`: 비허용 크레이트의 `IdentityConfirmationPrompt` 구현 차단 제거 | `prompt_trait_impl_alias_and_reexport_are_enforced_across_crates` |
| M19 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `PromptCheck::check_use_entry`: `IdentityConfirmationPrompt as` 별칭 차단 제거 | `prompt_trait_impl_alias_and_reexport_are_enforced_across_crates` |
| M20 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `PromptCheck::check_glob_reexport`: 모듈·Cargo 별칭 글롭 재수출 차단 제거 | `prompt_trait_impl_alias_and_reexport_are_enforced_across_crates` |
| M21 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `PromptCheck::check_macro_tokens`: 매크로 내부 `IdentityConfirmationPrompt impl` 차단 제거 | `test_module_impl_async_block_and_local_trait_are_rejected` |
| M22 | `crates/application/src/confirm_instrument_identity.rs` | `confirm_identifier_change`: 프롬프트 대기 전 시계 조기 조회 | `denied_and_unavailable_decisions_never_issue_confirmations_or_read_clock` |
| M23 | `crates/application/src/confirm_instrument_identity.rs` | `confirm_retirement_correction`: `Denied`를 `Unavailable`로 오매핑 | `denied_and_unavailable_decisions_never_issue_confirmations_or_read_clock` |
| M24 | `crates/application/src/confirm_instrument_identity.rs` | `confirm_link_correction`: `Confirmed` 판정을 `Unavailable`로 폐기 | `confirmed_link_correction_succeeds_through_use_case_and_memory_repository` |
| M25 | `crates/testkit-ports/src/scripted_identity_confirmation.rs` | `ScriptedIdentityConfirmation::confirm_retirement_correction`: 전체 호출 기록 누락 | `confirmed_retirement_correction_succeeds_through_use_case_and_memory_repository` |
| M26 | `crates/testkit-ports/src/scripted_identity_confirmation.rs` | `ScriptedIdentityConfirmationIssuer::issue`: `Unavailable`을 `Denied`로 오매핑 | `protected_scripted_issuer_enforces_decisions_and_opens_all_three_memory_write_paths` |

## 결과

- `domain`·`application`·`testkit-ports`·`xtask`·`harness`에 걸쳐 `UserIdentityConfirmation`의 발급 경계가 코드·타입·구조 검사·편집 보호 네 겹으로 고정된다.
- 기존 D-55 컴파일 실패 프로브 14건과 기존 테스트 단언은 한 줄도 변경하지 않는다.
- 후속 작업: **S-24**(이력·정정 영속 스키마·포트·저장소 구현), 확인 화면·`platform-macos` 어댑터 구현, H-20 로컬 잠금.
- 바뀌면 다시 볼 조건: 확인 화면 어댑터 위치(정의 대기 1번)가 `src-tauri`로 결정되거나, `ConfirmInstrumentIdentity`가 영속 포트를 직접 받도록 통합되거나(정의 대기 4번), `confirm_instrument_identity.rs`·`instrument_mapping_change.rs`를 `agent-writable.txt`에 편입할 때(정의 대기 3번).
