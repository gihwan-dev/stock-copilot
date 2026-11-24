# ADR h-66: H-64 3차 수정 독립 검증과 컴파일러 해석 기반 발급 호출 구조 검사

- 상태: 초안 (2026-11-24)
- 관련 작업: H-66 (연관 H-64, ADR 0162, D-55, ADR 0157)
- 반영한 파일: `crates/xtask/src/arch_check.rs`, `crates/xtask/src/arch_check/graph.rs`, `crates/xtask/src/arch_check/source.rs`, `crates/xtask/src/arch_check/source/identity_confirmation.rs`, `crates/xtask/src/arch_check/source/identity_confirmation/prompt_def.rs`, `crates/xtask/src/arch_check/source/identity_confirmation/tests.rs`, `crates/xtask/src/arch_check/structural.rs`, `crates/xtask/src/arch_check/structural/tests.rs`, `crates/xtask/src/source_check/tokens.rs`, `harness/arch-check/cases.toml`, `harness/arch-check/cases/identity-confirmation-h66-followup-bypasses-fail/**`, `harness/source-checks/cases.toml`, `harness/source-checks/sources/macro-collect-functions.rs`

## 배경

H-64([ADR 0162](0162-identity-confirmation-issuance.md))의 세 리뷰 회차 모두에서 매크로로 식별자를 합성해 소스 텍스트 기반 발급 호출 검사를 우회하는 경로가 나왔다(`paste!`, 전이 의존의 재수출 매크로, `#[serde(default = "...")]` 속성 문자열·이스케이프, 연관 타입). 3회차 수정분(`c405d81fc25484a3af61c73f4b82ed7c9f0e5280`)은 회차 상한 때문에 독립 리뷰를 받지 못한 상태였다.

H-66은 두 가지를 수행한다.
1. `c405d81f`의 3회차 수정분을 독립 적대적 리뷰로 검증하고 남은 우회를 닫는다.
2. 발급 함수(`domain::instrument_mapping_change::UserIdentityConfirmation::issue_after_user_confirmation`) 호출 위치 검사를 소스 글자 금지 목록 대신 컴파일러가 매크로 확장·이름 해석·타입 정규화까지 마친 결과 기준으로 옮겨, 새 매크로가 생겨도 금지 목록을 늘릴 필요가 없게 한다.

## 1. H-64 3차 수정(`c405d81f`) 독립 검증과 후속 보강

새 Claude Opus 5.5 적대적 리뷰어에게 `4115c3fa..c405d81f` diff와 ADR 0162만 제공해 검증한 결과, 3회차 지적은 모두 닫혔음이 확인되었다. 추가로 코드 분석에서 5건(중요 1건, 사소 4건)이 제기되었고 다음과 같이 반영했다.

| 번호 | 등급 | 지적 내용 | 조치 |
|---|---|---|---|
| 1 | 중요 | `crates/domain/src/instrument_mapping_change.rs`에 래퍼 값 추출 메서드(`ConfirmedIdentifierChange::into_confirmation`, `IdentityChangeConfirmation::into_origin`)나 공개 필드를 추가하면 확인된 요청 A의 `origin`을 다른 요청 B에 재결합할 수 있고 `DefiningFileCheck`가 못 잡음 | `DefiningFileCheck`에 5개 확인 래퍼 구조체(`IdentityChangeConfirmation`, `ConfirmedIdentifierChange`, `MappingCorrectionConfirmation`, `ConfirmedRetirementCorrection`, `ConfirmedLinkCorrection`)의 비공개 필드·허용 derive·제네릭 부재, trait impl 금지, inherent impl의 `new` 외 메서드에 `&self` 수신자·비제네릭·소유권/콜백 반환·`&mut` 입력 금지, `IdentityConfirmationOrigin`의 `impl`·비허용 derive 금지, non-private 자유 함수/보조 구조체 및 타입 별칭·연관 타입의 확인 타입 노출 금지를 추가 |
| 2 | 사소 | `PromptCheck::check_macro_tokens`가 `use` 포함·`impl` 부재 시 검사를 건너뛰어 `custom_macro!(use AutoPrompt => IdentityConfirmationPrompt)` 통과 | 비-File/비-Expr 매크로 토큰의 `is_use_only` 예외 제거 |
| 3 | 사소 | `PromptCheck::visit_attribute`가 문자열 리터럴만 검사해 `#[some_attr(IdentityConfirmationPrompt)]` 통과 | `PromptCheck::visit_attribute`에 식별자 토큰 검사(`stream_contains_ident`) 추가 |
| 4 | 사소 | `may_touch_identity_confirmation` 사전 필터가 `"[<"`만 찾아 공백 포함 `[ <x y> ]`를 건너뛰고 `specta::_collect_functions`가 금지 목록에 없음 | 공백 허용 paste 괄호 사전 필터(`contains_paste_bracket_text`)와 `_collect_functions` 차단(`arch-check` 및 `source-check` + 회귀 예제) 추가 |
| 5 | 사소(정책 공백) | `platform-macos`의 `IdentityConfirmationPrompt` 구현 내용은 검사하지 않음(아직 구현체 없음, ADR 0162 정의 대기 1) | 기존 허용 케이스를 유지하고 아래 `## 남는 한계`에 명시(`just known-gaps` 집계 대상) |

## 2. 구조적 검사 설계 결정과 버린 대안

| 후보 | 판정 | 근거 |
|---|---|---|
| **(가) `xtask arch-check` 내부 컴파일러 해석 기반 구조적 검사 (`cargo clippy` + `--force-warn clippy::disallowed_methods` / `clippy::disallowed_types`)** | **선택** | 안정 툴체인(rustc 1.99.1 내장 clippy)만 사용해 새 외부 크레이트와 nightly 기능이 0개다. 컴파일러가 모든 매크로 확장·이름 해석·타입/연관 타입 정규화·`include!`·`build.rs` 생성 코드를 끝낸 뒤 실제 `DefId` 기준으로 발급 함수와 봉인 타입의 모든 참조를 JSON으로 보고한다. `--force-warn`이 소스 내 `#[allow]`·`#[expect]`·`#[automatically_derived]`를 모두 무력화한다. 전용 캐시(`target/arch-check-clippy`)로 일반 빌드 캐시 오염을 막고, `TAURI_CONFIG` 빈 `frontendDist`로 `apps/desktop/dist` 부재 시에도 `src-tauri`까지 검사한다 |
| **(나) 매크로 확장 뒤 토큰(`cargo expand` / `-Zunpretty=expanded`)에 AST 검사 적용** | 기각 | `-Zunpretty=expanded`는 nightly 전용(`RUSTC_BOOTSTRAP` 금지)이고 `cargo expand`는 새 외부 도구에 같은 nightly 의존을 가진다. 또한 확장 토큰만으로는 다른 크레이트의 연관 타입·타입 별칭 정규화를 해석하지 못한다 |
| **(다) 컴파일 산출물(`.rmeta`·MIR·오브젝트 심볼) 검사** | 기각 | `issue_after_user_confirmation`은 ZST를 반환하는 `pub const fn`이므로 상수 평가·인라이닝 후 오브젝트 심볼이 사라진다. `.rmeta`는 비공개 비안정 포맷이며, `--emit=mir`는 미인스턴스화 제네릭 및 함수 포인터 처리와 별도 파서 유지 비용이 크다 |
| **(라) 타입 설계로 호출 자체 좁히기(발급 함수가 `application` 포트 결과 타입을 증거 인자로 요구)** | 기각 | `domain`은 어떤 내부 크레이트에도 의존할 수 없다(`domain = []`). `domain`에 공개 증거 트레이트를 두면 모든 어댑터가 구현 가능해 위조를 못 막고, 확인 타입을 `application`으로 옮기면 D-55 도메인 불변식과 기존 컴파일 실패 프로브 14건이 깨진다 |

### 구조적 검사 동작 방식 (`crates/xtask/src/arch_check/structural.rs`)

1. **대상 패키지 동적 계산**: `workspace.graph.members_reaching(&defining_crate)`로 `Normal`/`Build`/`Dev` 전이 의존성 중 `domain`에 도달 가능한 모든 워크스페이스 멤버를 동적으로 선택한다(Linux에서는 런타임 `std::env::consts::OS == "linux"` 분기로 `harness/ci/linux/exclusions.json`의 macOS 전용 크레이트만 제외).
2. **전용 `clippy.toml`과 환경 격리**: `<target>/arch-check-clippy/conf/clippy.toml`에 정규 `DefPath`(`domain::instrument_mapping_change::UserIdentityConfirmation::issue_after_user_confirmation`, `domain::instrument_mapping_change::UserIdentityConfirmationSeal`)를 기록하고 내용 변경 시에만 덮어써 증분 캐시 mtime을 유지한다. 자식 `cargo clippy` 실행 시 23개 환경 변수(`RUSTFLAGS`·`CARGO_ENCODED_RUSTFLAGS`·`CARGO_TARGET_*_RUSTFLAGS`·`CARGO_TARGET_*_RUNNER`·`RUSTC`·`RUSTC_WRAPPER`·`CARGO_BUILD_*`·`CLIPPY_CONF_DIR`·`TAURI_CONFIG` 등)를 정리한 뒤 `RUSTFLAGS=""`·`RUSTC_WRAPPER=""`·`RUSTC_WORKSPACE_WRAPPER=""`·`CLIPPY_CONF_DIR`을 명시 고정하고, `cargo clippy` 하위 인자로 `--config build.rustc-wrapper=""`·`--config build.rustflags=[]`·`--config env.CLIPPY_CONF_DIR.{value,force=true}`·`--config env.RUSTFLAGS.{value,force=true}`·`--config env.RUSTC_WRAPPER.{value,force=true}`를 넘겨 `$CARGO_HOME/config.toml` 및 `CARGO_TARGET_*_RUSTFLAGS` 덮어쓰기까지 차단한다. `src-tauri`가 대상에 포함되면 `<target>/arch-check-clippy/empty-dist`를 `TAURI_CONFIG`로 넘겨 프론트엔드 빌드 전에도 `src-tauri`를 검사한다.
3. **엄격한 허용 위치·패키지·타깃·열(column) 교차 검증**:
   - `clippy::disallowed_methods`: `span.expansion`이 없고(`null`), Cargo 메시지의 `manifest_path` 소속 패키지 디렉터리 안이며 `target.kind`가 `"lib"`이고, (a) `defining_file`의 `#[cfg(test)] mod tests` 범위 또는 (b) `issuance_allowed_files`의 AST 직접 호출(`ExprCall` callee 경로)의 **줄+열(`SpanRange`)** 범위 안일 때만 허용한다. 그 외 모든 참조(매크로 확장, `OUT_DIR`, 교차 크레이트/비-`lib` `include!` 스푸핑, 허용 파일 같은 줄의 함수 포인터·매크로, 타 크레이트/파일, 통합 테스트·바이너리 타깃)는 `identity-confirmation-issuance` 위반으로 보고한다.
   - `clippy::disallowed_types`: `defining_file`의 `struct UserIdentityConfirmationSeal`(derive 확장 포함)·`struct UserIdentityConfirmation`·`#[cfg(test)] mod tests`와 `defining_test_file`의 `#[cfg(test)] mod tests`만 허용하고 그 외 모든 봉인 타입 참조(`SealForge` 연관 타입 포함)를 거부한다.
4. **Fail-Closed 보증과 표면 판별**: `cargo clippy` 비0 종료, 컴파일 에러, `defining_file` 소속 패키지 미식별, 대상 패키지 중 `compiler-artifact` 누락, 스팬 없는 진단, 봉인 타입 기준 진단 0건, 그리고 기대 발급 패키지별(`domain`·`application`·`testkit-ports`) 카나리 진단 누락 모두 검사 실패로 처리한다. `arch_check::command`는 실제 저장소(`xtask` 멤버 또는 `hashes.json` 또는 정의·허용 파일 존재)에서는 항상 구조적 검사를 강제해 정의 파일 삭제 시 실패하고, `domain`·`xtask`가 없는 기존 최소 의존성 테스트 픽스처(`command_fails_on_violation_and_passes_clean_workspace`)만 건너뛴다.
5. **기존 소스 글자/AST 검사 유지 및 1회차 보강**: 기존 AST/토큰 검사는 정의 파일 내부 변조(`ItemTrait`·트레이트 `impl`·`[ConfirmedIdentifierChange; 1]` 같은 비단일 `self_ty`·`pub` 자유 함수·`pub(crate)` 다중 인자 싱크·래퍼 재결합 함수·래퍼 `new`의 동일/외부 래퍼 인자 금지), 유스케이스 화이트리스트 결합(`use_case.rs`), 프롬프트 정의 파일의 요청 구조체 3종 필드·`new`·접근자·`into_parts` 1:1 투영 고정(`prompt_def.rs`), 프롬프트 구현·별칭·재수출(`PromptCheck`), 보호 가짜 규칙을 계속 강제하며, 12건의 기존 `arch-check` 예제와 컴파일 실패 프로브(D-55 14건 + H-64 7건)를 그대로 유지한다.

## 3. 구조적 검사가 잡는 것과 못 잡는 것

| 경로 / 위협 | 구조적 검사(`structural.rs`) | 보조 방어선 |
|---|---|---|
| 금지 목록에 없는 새 절차 매크로 식별자 합성 (`brand_new_unlisted_macro!`) | **잡음** (컴파일러 `DefId` 해석) | `source-check` 워크스페이스 proc-macro 금지 + `deny.toml` 보호 |
| `paste!`·`specta::internal::paste!`·`fn_datatype!`·`_collect_functions!` | **잡음** | `arch-check` `ReexportedPasteCheck` + `source-check` `BANNED_MACROS` |
| `#[derive(serde::Deserialize)] #[serde(default = "...")]` (원문·`\x..`·`\u{..}` 이스케이프) | **잡음** | `arch-check` 속성 리터럴 언이스케이프 검사 |
| `macro_rules!`·`include!`·`build.rs` 생성 코드(`OUT_DIR`) | **잡음** | `source-check` 매크로·`build.rs` 금지 |
| 교차 크레이트 또는 비-`lib` 타깃에서의 허용 파일 `include!` 스푸핑 | **잡음** (`manifest_path`·`target.kind` 대조) | `source-check` `include!` 금지 |
| 함수 포인터·트레이트 객체(`&dyn Fn`)·제네릭 콜백·`const`/`static`·연관 타입(`<T as Alias>::Out`)·타 크레이트 재수출/간접 호출 | **잡음** | `arch-check` AST/토큰 검사 |
| `#[allow(clippy::all, clippy::disallowed_methods, warnings)]` / `#[expect(...)]` | **잡음** (`--force-warn`) | `source-check` `allow` 금지 + `disable-check` |
| `defining_file` 제품 코드 내 발급 호출 / `SealForge` 연관 타입(`type Seal = UserIdentityConfirmationSeal`) | **잡음** | `DefiningFileCheck` AST 검사 |
| 허용 파일(`confirm_instrument_identity.rs`) 내부 같은 줄 함수 포인터·매크로 확장 호출 | **잡음** (`expansion == null` + 직접 `ExprCall` 줄·열 `SpanRange` 대조) | `use_case.rs` 화이트리스트 + `ProtectedFakeCheck` + 보호 파일 해시 |
| 통합 테스트(`tests/*.rs`)·바이너리·예제·벤치 타깃의 무단 호출 | **잡음** (`--all-targets --all-features`) | `arch-check` 전체 `.rs` 스캔 |
| 절차 매크로가 `#[cfg(not(clippy))]`를 방출해 clippy 빌드에서 호출을 숨기는 경우 | **못 잡음** (clippy는 `--cfg clippy`로 컴파일) | `source-check` 워크스페이스 proc-macro·`macro_rules!`·`cfg_not_test` 금지 + `deny.toml` 보호 |
| `defining_file` 내부에서 타입 표기 없이 값 생성자 `UserIdentityConfirmation(UserIdentityConfirmationSeal)`만 호출하는 경우 | **못 잡음** (`disallowed_types`는 타입 위치만 검사) | 외부 크레이트는 Rust 모듈 비공개성(`E0603`)으로 원천 불가; `defining_file` 내부는 `DefiningFileCheck` AST 검사로 차단 |
| 임의 절차 매크로로 `impl IdentityConfirmationPrompt for ...` 생성 | **못 잡음** (`disallowed_types`는 trait impl에 미발동) | `PromptCheck` AST/토큰 검사 + `prompt_def.rs` + `source-check` proc-macro 금지 + `deny.toml` 보호 |
| 허용 파일 내부의 논리적 결합(요청 치환·`Denied` 분기 발급·루프 다중 발급) | 범위 밖(호출 위치 검사) | `use_case.rs` 화이트리스트 AST 검사 + `ProtectedFakeCheck` + 보호 파일 해시 |

## 4. 검사 시간 실측 및 손 변형

### 격리 복사본 손 변형 (H-66 범위 26/26 검출, H-65 포함 묶음 32/32 검출)

| 번호 | 대상 파일 | 무력화한 조건 | 검출 테스트 |
|---|---|---|---|
| M06 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `all_fields_private`: 래퍼 구조체 비공개 필드 검사 제거 | `h66_defining_file_wrapper_extraction_is_blocked` |
| M07 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `check_impl_constraints`: 확인 타입 관련 trait impl 차단 제거 | `h66_r2_defining_file_trait_rebind_and_sink_bypasses_are_blocked` |
| M08 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `type_is_shared_ref`: 소유권 반환 타입 검사 제거 | `h66_r2_defining_file_trait_rebind_and_sink_bypasses_are_blocked` |
| M09 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `signature_exposes_owned_confirmation`: `&mut` 입력·수신자 검사 제거 | `h66_r2_defining_file_trait_rebind_and_sink_bypasses_are_blocked` |
| M10 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `DefiningFileCheck::visit_item_fn`: 공개/다중인자 자유 함수 추출 검사 제거 | `h66_defining_file_wrapper_extraction_is_blocked` |
| M11 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `DefiningFileCheck::visit_item_enum`: `IdentityConfirmationOrigin` 변조 검사 제거 | `h66_defining_file_wrapper_extraction_is_blocked` |
| M12 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `PromptCheck::visit_attribute`: 식별자 토큰 검사 제거 | `h66_prompt_macro_attr_and_spaced_paste_bypasses_are_blocked` |
| M13 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `contains_paste_bracket_text`: 공백 포함 `[ <` 사전 필터 제거 | `h66_prompt_macro_attr_and_spaced_paste_bypasses_are_blocked` |
| M14 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `BANNED_PASTE_MACROS`: `_collect_functions` 차단 제거 | `h66_prompt_macro_attr_and_spaced_paste_bypasses_are_blocked` |
| M15 | `crates/xtask/src/source_check/tokens.rs` | `BANNED_MACROS`: `_collect_functions` 차단 제거 | `source_check::tests::source_examples_report_exactly_their_rules` |
| M16 | `crates/xtask/src/arch_check/structural.rs` | `is_allowed_issuer_span`: 매크로 확장 span 차단 제거 | `structural_parser_isolates_expansion_defining_line_allowed_line_and_seal_line_guards` |
| M17 | `crates/xtask/src/arch_check/structural.rs` | `is_allowed_issuer_span`: 교차 패키지 소속 검사 제거 | `structural_parser_rejects_cross_package_and_non_lib_target_spoofing` |
| M18 | `crates/xtask/src/arch_check/structural.rs` | `span_belongs_to_package`: `lib` 타깃 강제 제거 | `structural_parser_rejects_cross_package_and_non_lib_target_spoofing` |
| M19 | `crates/xtask/src/arch_check/structural.rs` | `is_allowed_issuer_span`: 정의 파일 `#[cfg(test)]` 범위 제한 제거 | `structural_parser_isolates_expansion_defining_line_allowed_line_and_seal_line_guards` |
| M20 | `crates/xtask/src/arch_check/structural.rs` | `is_allowed_issuer_span`: 허용 파일 직접 호출 줄+열 `SpanRange` 제한 제거 | `structural_parser_isolates_expansion_defining_line_allowed_line_and_seal_line_guards` |
| M21 | `crates/xtask/src/arch_check/structural.rs` | `is_allowed_seal_span`: 정의 파일 봉인 타입 허용 범위 제한 제거 | `structural_parser_isolates_expansion_defining_line_allowed_line_and_seal_line_guards` |
| M22 | `crates/xtask/src/arch_check/structural.rs` | `validate_parsed_output`: clippy 비0 종료 실패 처리 제거 | `structural_parser_fails_closed_on_errors_missing_artifacts_and_canary` |
| M23 | `crates/xtask/src/arch_check/structural.rs` | `validate_parsed_output`: 누락 아티팩트 실패 처리 제거 | `structural_parser_fails_closed_on_errors_missing_artifacts_and_canary` |
| M24 | `crates/xtask/src/arch_check/structural.rs` | `validate_parsed_output`: 봉인 타입 카나리 검사 제거 | `structural_parser_fails_closed_on_errors_missing_artifacts_and_canary` |
| M25 | `crates/xtask/src/arch_check/structural.rs` | `validate_parsed_output`: 패키지별 발급 함수 카나리 검사 제거 | `structural_parser_fails_closed_on_errors_missing_artifacts_and_canary` |
| M26 | `crates/xtask/src/arch_check/structural.rs` | `evaluate_diagnostic`: 빈 스팬 위반 보고 제거 | `structural_parser_reports_empty_spans_as_violation` |
| M28 | `crates/xtask/src/arch_check/structural.rs` | `SpanRange::contains`: 열(column) 경계 검사 제거 | `structural_parser_isolates_expansion_defining_line_allowed_line_and_seal_line_guards` |
| M29 | `crates/xtask/src/arch_check/structural.rs` | `push_clippy_config_overrides`: `--config` 덮어쓰기 차단 제거 | `structural_check_catches_same_line_fn_pointer_and_ignores_hostile_cargo_env` |
| M30 | `crates/xtask/src/arch_check/source/identity_confirmation.rs` | `non_wrapper_fn_violates_confirmation_boundary`: `pub(crate)` 다중 인자 싱크 차단 제거 | `h66_r2_defining_file_trait_rebind_and_sink_bypasses_are_blocked` |
| M31 | `crates/xtask/src/arch_check/source/identity_confirmation/prompt_def.rs` | `verify_implemented_structs`: 요청 구조체 표준 필드 대조 제거 | `h66_r2_prompt_def_blocks_hidden_fields_extra_constructors_and_into_parts_swaps` |
| M32 | `crates/xtask/src/arch_check/source/identity_confirmation/prompt_def.rs` | `is_canonical_prompt_into_parts`: 튜플 필드 순서 대조 제거 | `h66_r2_prompt_def_blocks_hidden_fields_extra_constructors_and_into_parts_swaps` |

### 검사 시간 실측

- `arch-check` 구조적 clippy 패스(7개 `domain` 의존 크레이트, 전용 `target/arch-check-clippy` 캐시): 첫 콜드 실행 약 47.6초(`src-tauri` 포함), 이후 증분 warm 실행 **2.65–2.73초**(동시 중부하 환경 `just arch-check` 전체 15.86초). 일반 `target/debug` 캐시와 분리되어 `just clippy`·`nextest` 재빌드를 유발하지 않는다.

## 남는 한계

- 절차 매크로가 확장 결과에 `#[cfg(not(clippy))]`를 방출하면 clippy 구조적 검사가 해당 호출을 보지 못한다(현재는 `source-check`의 워크스페이스 proc-macro·`macro_rules!`·`cfg_not_test` 금지와 보호된 `deny.toml` 허용 목록으로 막고 있다).
- `clippy::disallowed_types`는 값 생성자 표현식과 트레이트 `impl`에는 발동하지 않으므로, `defining_file` 내부의 값 생성자 변조와 비허용 크레이트의 `impl IdentityConfirmationPrompt`는 계속 AST 검사(`DefiningFileCheck`, `PromptCheck`, `prompt_def.rs`)에 의존한다.
- `crates/adapters/platform-macos`는 `prompt-impl-crates`에 허용되어 있고 에이전트 쓰기 가능 경로에 속하지만, 실제 확인 화면 구현이 확정(ADR 0162 정의 대기 1·2)되기 전까지 구현 내용 검사(`AlwaysConfirm`·제네릭 `FnPrompt<F>` 차단)가 아직 없다.
- Linux CI에서는 `harness/ci/linux/exclusions.json`에 따라 macOS 전용 5개 크레이트(`src-tauri`, `e2e-app`, `platform-macos`, `secrets-keychain`, `store-sqlite`)가 구조적 clippy 대상에서 빠지며, 해당 크레이트들의 구조적 검증은 macOS 실행·병합 검사에서 수행된다.
- 로컬 증분 실행 시 `target/arch-check-clippy/debug/.fingerprint/*/output-*` 진단 캐시 파일 자체를 외부에서 직접 변조하면 Cargo가 변조된 캐시를 재생할 수 있다(로컬 한정 한계이며, 캐시를 복원하지 않는 원격 CI 및 `just fresh-all`에서는 재현되지 않는다).
