# ADR D-55(초안): 확인된 종목 식별자 변경의 이력 연결과 발급원 강제

- 상태: 초안

## 결정

### 1. 확인된 종목 식별자 변경의 이력 연결

종목명·심볼 변경, 액면분할·병합에 따른 ISIN·코드 변경 등 외부 식별자 변경은 `InstrumentMappings::link_confirmed_change`로만 기존 내부 ID(`InstrumentId`)의 이력에 연결한다. 자동 추정이나 브로커 관측·AI 제안만으로는 연결하지 않고 `IdentityUncertain`을 유지한다.

- **확인의 출처(`IdentityConfirmationOrigin`)**: `UserConfirmed`, `BrokerObservation`, `AiSuggestion`을 구분한다. 현재 보수적 기본값은 `UserConfirmed`만 승인하고 나머지는 `MappingTransitionError::IdentityUncertain`으로 거부한다.
- **변경 증거(`IdentityChangeEvidence`)**: `None`, `NoteOnly`, `DisclosureOnly`, `ActionOnly`, `DisclosureAndAction`을 구분한다. 현재 보수적 기본값은 공시 키(`DisclosureKey`)·기업행위(`CorporateAction`)·사용자 사유(`UserReason`)가 모두 결합된 `DisclosureAndAction`만 인정하며, 부족한 증거는 `IdentityUncertain`으로 거부한다.
- **기업행위 정합성**: `CorporateAction`의 종목 ID가 변경 대상 내부 ID와 같아야 하고, `CorporateActionKind::Delisting`은 식별자 변경 증거로 거부한다(`InvalidEvidence`). `CorporateActionKind::SymbolChange { from, to }`인 경우 변경 전후 `InstrumentSource`의 심볼과 대소문자 무시 기준으로 일치해야 한다(`InvalidEvidence`).
- **상태 전이와 보존**: 변경이 승인되면 변경 전 `InstrumentSource`는 `InstrumentMappingStatus::Superseded`로 전이하고, 변경 후 `InstrumentSource`는 같은 `InstrumentId`에 `Active`로 추가된다. 이후 변경 전 원본의 `resolve`·`ensure`·`retire`는 모두 `InstrumentMappingError::Superseded`를 반환하며, `lookup`과 `for_id`는 과거 이력을 변경 없이 조회한다.
- **충돌·분기·순환 방지**:
  - `previous`와 `next`가 정확히 같거나 대소문자 무시 기준으로 `(시장, 심볼, ISIN)`이 모두 같으면 `UnchangedSource`, 시장 변경은 `MarketMismatch`.
  - `previous`가 없거나 다른 ID에 묶여 있거나 `Active`가 아니면(`Superseded`/`Retired`) 각각 `Missing`, `SourceBoundToOtherId`, `SourceNotActive`. `previous`가 기존 원본과 심볼/ISIN이 부분 겹침이면 `IdentityUncertain`.
  - `next`가 이미 다른 ID에 묶여 있으면 `SourceBoundToOtherId`, 같은 ID에 이미 기록되어 있거나 대소문자만 다른 기존 원본이면(순환 포함) `SourceAlreadyRecorded`, 다른 ID의 기존 원본과 심볼 또는 ISIN이 대소문자 무시 기준으로 겹치면 `IdentityUncertain`.
- **폐지·연결 오기록 정정**:
  - 오기록 정정도 `UserConfirmed` + `MappingCorrectionEvidence::DisclosureAndNote`(공시 키와 사용자 사유)를 요구한다. 부족하면 `IdentityUncertain`으로 거부한다.
  - `correct_retirement`: `Retired` 상태인 해당 ID의 원본을 `Active`로 복구한다(`Active`/`Superseded`는 `SourceNotRetired`).
  - `correct_link`: 보수적 기본값으로 해당 ID의 **가장 최근 활성 연결**(LIFO 끝단이며 `mistaken_next`가 현재 `Active`)만 되돌린다. 되돌리면 `previous`는 `Active`로 복원되고 잘못 연결된 `mistaken_next`는 매핑 레코드에서 분리되어 이후 독립 발급·재판정이 가능하다. 비최신이거나 `mistaken_next`가 `Retired`이면 `LinkNotLatest`로 거부한다.
- **감사 이력(`MappingHistoryEvent`)**: 모든 승인된 변경(`IdentifierChanged`), 폐지 정정(`RetirementCorrected`), 연결 정정(`LinkCorrected`)은 `InstrumentMappings::history` / `history_for_id`에 불변 추가(append-only)로 남긴다. 실패한 호출은 원장 상태를 전혀 변경하지 않는다.
- **마스터 투영(`InstrumentMaster`)**: `InstrumentMasterEntry::Unavailable`에 `InstrumentMappingError::Superseded`를 유효한 불가 사유로 추가했다.

| 상황 | 판정 |
| --- | --- |
| `BrokerObservation` 또는 `AiSuggestion` 출처로 변경·정정 요청 | `IdentityUncertain`, 상태 불변 |
| 변경 증거가 `None`/`NoteOnly`/`DisclosureOnly`/`ActionOnly` | `IdentityUncertain`, 상태 불변 |
| 정정 증거가 `None`/`NoteOnly` | `IdentityUncertain`, 상태 불변 |
| 기업행위 종목 ID 불일치, `Delisting`, 또는 `SymbolChange` 심볼 불일치 | `InvalidEvidence`, 상태 불변 |
| 동일 원본 변경(`previous == next`) 또는 시장 변경(`Kr` ↔ `Us`) | `UnchangedSource` / `MarketMismatch`, 상태 불변 |
| `previous`가 `Superseded` 또는 `Retired` 상태에서 변경 연결 | `SourceNotActive`, 상태 불변 |
| `next`가 이미 다른 ID에 묶임 | `SourceBoundToOtherId`, 상태 불변 |
| `next`가 같은 ID의 기존 이력에 이미 존재(순환 포함) | `SourceAlreadyRecorded`, 상태 불변 |
| `next`가 다른 ID의 기존 원본과 심볼 또는 ISIN이 겹침 | `IdentityUncertain`, 상태 불변 |
| 사용자 확인 + 공시·기업행위·사유 충족, 활성 원본에서 비중복 새 원본으로 연결 | 기존 `previous`는 `Superseded`, `next`는 같은 ID의 `Active`, `IdentifierChanged` 이력 추가 |
| 교체된(`Superseded`) 원본의 `resolve`·`ensure`·`retire` | `InstrumentMappingError::Superseded`, 상태 불변 |
| `Retired` 원본의 사용자 확인 폐지 정정(`DisclosureAndNote`) | 해당 원본 `Active` 복원, `RetirementCorrected` 이력 추가 |
| 가장 최근 활성 연결의 사용자 확인 연결 정정(`DisclosureAndNote`) | `previous` `Active` 복원, `mistaken_next` 분리, `LinkCorrected` 이력 추가 |
| 비최신 연결 또는 끝단이 `Retired`인 연결의 정정 요청 | `LinkNotLatest`, 상태 불변 |

### 2. 독립 발급원 타입 강제와 기존 ID 편입·전역 유일성 검증

- **봉인된 후보 타입(`IssuedInstrumentId`)**: 내부 필드가 비공개이고 `Clone`, `Copy`, `Default`, `From<InstrumentId>`와 공개 생성자가 없는 일회성 소유권 토큰이다. 오직 `IndependentInstrumentIdIssuer::issue`만 이 값을 만들 수 있으며, `InstrumentMappings::ensure`와 `InstrumentMappingRepository::ensure`는 후보를 `IssuedInstrumentId` 값으로 소유권을 소비해 받는다.
- **컴파일 시간 차단 증명**: `Symbol`, `IsinCode`, 임의로 생성한 `InstrumentId`를 후보로 전달하거나, `IssuedInstrumentId(...)` 튜플 생성·`try_new`·`From`·`clone`·소비된 후보 재사용·`IndependentInstrumentIdIssuer::clone` 시도는 모두 컴파일 실패한다(`crates/domain/tests/instrument_issuer_boundaries.rs`, `crates/application/tests/instrument_issuer_boundaries.rs`의 `testkit::check_consumer` 양성 대조 + 음성 프로브로 검증).
- **기존 ID 편입과 예약**:
  - `IndependentInstrumentIdIssuer::from_existing` / `adopt_existing`는 기존 레거시 ID와 이미 발급된 순번 ID를 예약 집합에 편입하고, `issue`는 예약된 ID를 건너뛰며 순번이 `u64::MAX`를 소진하면 `InstrumentIdIssueError::Exhausted`를 반환한다.
  - `InstrumentMappings::adopt_existing_id` / `id_reservation`은 원장 외부에서 이미 존재하는 ID를 예약 집합에 편입해, 미연동 발급원이 같은 ID를 제안해도 `IdAlreadyUsed`로 거부한다.
- **가짜(`testkit-ports`)의 전역 유일성 검증**: `MemoryGlobalInstrumentIds`, `MemoryInstrumentIdIssuer`, `MemoryInstrumentMappingRepository`는 공유 전역 레지스트리를 통해 기존 ID 편입, 여러 원장 간·동시 스레드 간 ID 충돌 차단, 폐지된 ID의 타 원장 재발급 차단, 실패한 `ensure`의 후보 미예약, 미폴링(lazy) future의 비변이를 검증한다.

### 3. D-52 포트(`application`) 최소 변경 내역 (S-21 병합 참고)

S-21(`crates/adapters/store-sqlite`)과의 병합 충돌을 최소화하기 위해 `application::instrument_mapping`의 변경을 아래 2개 서명으로 제한했고, 신규 이력 변경·정정 메서드는 아직 포트에 추가하지 않았다(아래 "정의 대기" 항목 6 참고).

| 포트 / 타입 | 변경 전 | 변경 후 | 비고 |
| --- | --- | --- | --- |
| `InstrumentIdIssuer::issue` | `Result<InstrumentId, SystemPortError>` | `Result<IssuedInstrumentId, SystemPortError>` | 발급원 출처를 타입으로 강제 |
| `InstrumentMappingRepository::ensure` | `candidate: &InstrumentId` | `candidate: IssuedInstrumentId` | 후보의 일회성 소비와 발급원 강제 |
| `domain::InstrumentMappingStatus` | `Active`, `Retired` | `Active`, `Superseded`, `Retired` | SQLite 저장 상태 코드에 `Superseded` 추가 필요 |
| `domain::InstrumentMappingError` | `Missing`, `IdentityUncertain`, `Retired`, `IdAlreadyUsed` | `+ Superseded` | 마스터/조회 오류 매핑에 `Superseded` 추가 필요 |

## 정의 대기

아래 정책은 기존 문서가 확정하지 않았으므로 임의로 확정하지 않았다. 현재 구현은 확인·증거가 부족하면 `IdentityUncertain`을 유지하고 가장 보수적인 범위만 허용하는 안전한 기본값을 따른다.

| 항목 | 현재 안전한 기본값 | 선택지 | 사용자 영향 |
| --- | --- | --- | --- |
| 1. 변경 확인의 최소 증거 | `UserConfirmed` + `DisclosureAndAction`(공시 키·기업행위·사유 모두 필수) | A) 공시+기업행위+사유 필수(현재) / B) 공시+사유 허용(기업행위 미수집 시) / C) 공시 없는 사유만 허용 | A는 오연결 위험이 가장 낮으나 공시·기업행위 수집 전에는 수동 연결이 막힌다. B·C로 완화할수록 공시 지연 시 즉시 처리 가능하나 잘못된 수동 연결 위험이 커진다. |
| 2. 정정 확인의 최소 증거 | `UserConfirmed` + `DisclosureAndNote`(공시 키·사유 필수) | A) 공시+사유 필수(현재) / B) 사용자 사유만으로 오입력 정정 허용 | A는 근거 없는 되돌림을 막지만 사용자가 단순 오타를 직후 정정할 때도 공시 키를 넣어야 한다. B는 오타 복구가 빠르나 감사 근거가 약해진다. |
| 3. 연결 정정 허용 범위 | 해당 ID의 가장 최근 활성 연결(LIFO 끝단)만 정정, 끝단이 폐지 상태면 거부 | A) 최신 활성 연결만 정정(현재) / B) 연결 이후 주문·포지션·평가 참조가 없을 때만 허용 / C) 중간 이력이나 폐지 후 이력도 정정 허용 | A는 이력 그래프가 깨지지 않고 단순하다. B는 실거래 참조 이후 소급 변경을 방지하지만 하위 저장소 교차 검사가 필요하다. C는 과거 오류를 넓게 고칠 수 있으나 손익·원가 이력 정합성이 깨질 위험이 크다. |
| 4. 동일 심볼 재사용(폐지 후 타 법인 재상장) 처리 | 폐지 이력과 겹치면 `IdentityUncertain` 유지, 자동 재발급 없음 | A) 불확정 유지(현재) / B) 사용자 확인으로 "신규 별개 법인" 판정 후 새 내부 ID 발급 허용 | A는 합병·재상장 혼동을 원천 차단하지만 한국/미국 시장에서 과거 상장폐지 심볼이 새 법인에 재할당될 때 수동 해소가 필요하다. B는 재사용 심볼을 정상 편입할 수 있으나 별도 확인 절차와 화면이 필요하다. |
| 5. 시장 이동·이전 원본 재사용(순환) | 시장 불일치(`MarketMismatch`)와 기존 원본 재연결(`SourceAlreadyRecorded`) 거부 | A) 동일 시장·비순환만 허용(현재) / B) 상장 시장 이전(예: 이전상장)이나 과거 심볼 복귀를 명시적 예외로 허용 | A는 시장 경계·이력 단조성을 지킨다. B는 드문 이전상장·심볼 복귀를 지원하지만 과거 이력 상태 조회 규칙이 복잡해진다. |
| 6. 영속 포트(`InstrumentMappingRepository`) 확장 형태 | 기존 D-52 포트 5개 메서드 유지(S-21 충돌 최소화), 이력 변경·정정은 도메인·메모리 가짜 우선 구현 | A) `InstrumentMappingRepository`에 변경·정정·기존 ID 편입 메서드 추가 / B) 별도 쓰기 포트(`InstrumentIdentityCorrectionPort`)로 분리 | A는 트랜잭션 경계가 단일 저장소에 모인다. B는 조회·일반 동기화 경로와 사용자 확인 정정 경로의 권한을 타입 수준에서 분리할 수 있다. |
| 7. 독립 발급원 영속 형식과 직렬화 경계 | `IndependentInstrumentIdIssuer`는 결정론적 불투명 순번(`inst-{:016x}`) 발급 | A) SQLite 단조 시퀀스 테이블 기반 영속 순번 / B) UUIDv7·ULID 등 독립 식별자 + DB `UNIQUE` 제약 + 기존 ID 편입 | A는 짧고 결정적이며 테스트 재현성이 높다. B는 다중 원장 병합 시 충돌 가능성이 낮으나 저장소 복원 시 전역 유일성 검증을 계속 유지해야 한다. |

## 기존 테스트 기계적 수정 기록

`ensure`의 후보 타입을 `&InstrumentId`에서 소유권 소비형 `IssuedInstrumentId`로 강제하고 `InstrumentIdIssuer::issue` 반환 타입을 `IssuedInstrumentId`로 바꿈에 따라, 아래 기존 테스트 5개 파일의 호출부·준비 코드만 기계적으로 조정했다. 기존 테스트의 단언과 기대 결과는 하나도 삭제하거나 약하게 만들지 않았다.

| 파일 | 변경 대상 테스트 / 헬퍼 | 전 → 후 | 변경 전 SHA-256 (`0b223855`) | 변경 후 SHA-256 |
| --- | --- | --- | --- | --- |
| `crates/domain/tests/instrument_mapping.rs` | 헬퍼 `issued`/`id` 및 기존 7개 테스트의 `ensure` 호출부 | `ensure(&source, &id("..."))` → `ensure(&source, issued("..."))` (`IndependentInstrumentIdIssuer` 결정론 슬롯 발급) | `c5174afbd1b7a02a58ec8488d465aacb5ef8ec92993ba557c2f43415b525d792` | `e7f43fcc8c2c59cc7175a24169cf653faea65086ff178db0c0b2f56e8d68c893` |
| `crates/domain/tests/instrument_mapping_review.rs` | 헬퍼 `issued_slot`/`issued`/`id` 및 기존 3개 테스트의 `ensure` 호출부 | `ensure(&source, &id("..."))` → `ensure(&source, issued("..."))` (상태 기계 모델의 후보 슬롯 일치 유지) | `4939f791d128f65061dccba47610384c340375bee9e80cf9c290487c8c64fb44` | `2d150c00cb908a9f06cb32c7b16829d4699d995b838f830fb6b9b04891fa2e12` |
| `crates/testkit-ports/tests/instrument_mapping.rs` | 헬퍼 `issued_candidate`/`id` 및 기존 6개 테스트의 `ensure` 호출부 | `repo.ensure(&source, &id("..."))` → `repo.ensure(&source, issued_candidate("..."))` | `7d5ac0e0022c06992c76aa4a3e3338767f17ddae8738672d5c47592c4d3fe430` | `19715bcf8ee91efe7094acd8c6ec3346fb8ba5a36eace75bf3c58286970ae205` |
| `crates/testkit-ports/tests/instrument_mapping_review.rs` | `repository_reads_retired_history_without_reactivating_or_writing`의 `ensure` 준비 | `InstrumentId::try_new` 후보 → `IndependentInstrumentIdIssuer::new().issue()` 후보 | `ee6fdb694be65b70dd03956508e5c472fe6bfca38c8aae2ba795a0718b1d1aeb` | `84a85fa20066f76cc06bf01c738732216f728df338aca1b3bf34d6492628` |
| `crates/adapters/broker-toss/tests/t21_instruments.rs` | `Session::ensure` 서명, 헬퍼 `issued`/`id`/`issuer` 및 10개 seed `ensure` 호출부 | `candidate: &InstrumentId` → `candidate: domain::IssuedInstrumentId`, `ensure(..., &id(...))` → `ensure(..., issued(...))` | `560a4ea939f671ee49f0f2381e54c62efcd0b1190d2f0e65860dd33e48c33068` | `3fc0def2f3cca21ea9e42ec4fa86ff85c457567fc2b62c3e4550e52e6368f26a` |

`just test-diff 0b2238554479d46b69be508c4375d2d09b0ea8e4` 정확한 출력 (총 30건: `support-modified` 7건, `modified` 23건; `removed`/`support-removed`/`shadowing`/`not-listed`/`count-decreased`/`unreachable` 0건):

```text
broker-toss::t21_instruments tests: test-diff:support-modified:broker-toss::t21_instruments tests::fn id
broker-toss::t21_instruments tests: test-diff:support-modified:broker-toss::t21_instruments tests::fn issuer
broker-toss::t21_instruments tests: test-diff:support-modified:broker-toss::t21_instruments tests::impl impl InstrumentMappingRepository for Session <'_ >
broker-toss::t21_instruments tests::existing_ids_are_reused_without_consuming_candidates: test-diff:modified:broker-toss::t21_instruments tests::existing_ids_are_reused_without_consuming_candidates
broker-toss::t21_instruments tests::retired_and_collision_results_are_preserved_per_instrument: test-diff:modified:broker-toss::t21_instruments tests::retired_and_collision_results_are_preserved_per_instrument
broker-toss::t21_instruments tests::review2_uncertain_retirement_preserves_the_existing_mapping: test-diff:modified:broker-toss::t21_instruments tests::review2_uncertain_retirement_preserves_the_existing_mapping
broker-toss::t21_instruments tests::review3_real_ledger_rejects_uncertain_retirement_without_changing_history: test-diff:modified:broker-toss::t21_instruments tests::review3_real_ledger_rejects_uncertain_retirement_without_changing_history
broker-toss::t21_instruments tests::review_regression_verified_delisting_retires_existing_mapping: test-diff:modified:broker-toss::t21_instruments tests::review_regression_verified_delisting_retires_existing_mapping
broker-toss::t21_instruments tests::review_retirement_failure_preserves_the_system_error_and_existing_mapping: test-diff:modified:broker-toss::t21_instruments tests::review_retirement_failure_preserves_the_system_error_and_existing_mapping
broker-toss::t21_instruments tests::uncertain_identity_is_kept_in_the_result_without_losing_other_rows: test-diff:modified:broker-toss::t21_instruments tests::uncertain_identity_is_kept_in_the_result_without_losing_other_rows
broker-toss::t21_instruments tests::us_exchanges_preserve_source_case_and_use_the_existing_id: test-diff:modified:broker-toss::t21_instruments tests::us_exchanges_preserve_source_case_and_use_the_existing_id
domain::instrument_mapping tests: test-diff:support-modified:domain::instrument_mapping tests::fn id
domain::instrument_mapping tests::case_variants_never_become_distinct_new_sources: test-diff:modified:domain::instrument_mapping tests::case_variants_never_become_distinct_new_sources
domain::instrument_mapping tests::delisting_is_permanent_and_relisting_requires_external_resolution: test-diff:modified:domain::instrument_mapping tests::delisting_is_permanent_and_relisting_requires_external_resolution
domain::instrument_mapping tests::mapping_preserves_each_source_field_and_uses_existing_id: test-diff:modified:domain::instrument_mapping tests::mapping_preserves_each_source_field_and_uses_existing_id
domain::instrument_mapping tests::mapping_properties_hold_with_fixed_independent_candidates: test-diff:modified:domain::instrument_mapping tests::mapping_properties_hold_with_fixed_independent_candidates
domain::instrument_mapping tests::retiring_an_unrelated_source_leaves_the_existing_mapping_unchanged: test-diff:modified:domain::instrument_mapping tests::retiring_an_unrelated_source_leaves_the_existing_mapping_unchanged
domain::instrument_mapping tests::same_symbol_in_different_markets_needs_a_distinct_isin_and_candidate: test-diff:modified:domain::instrument_mapping tests::same_symbol_in_different_markets_needs_a_distinct_isin_and_candidate
domain::instrument_mapping tests::uncertain_changes_fail_without_mutating_or_reusing_candidates: test-diff:modified:domain::instrument_mapping tests::uncertain_changes_fail_without_mutating_or_reusing_candidates
domain::instrument_mapping_review tests: test-diff:support-modified:domain::instrument_mapping_review tests::fn id
domain::instrument_mapping_review tests: test-diff:support-modified:domain::instrument_mapping_review tests::impl impl ReferenceCase
domain::instrument_mapping_review tests::read_only_lookup_keeps_identity_and_status_after_delisting: test-diff:modified:domain::instrument_mapping_review tests::read_only_lookup_keeps_identity_and_status_after_delisting
testkit-ports::instrument_mapping tests: test-diff:support-modified:testkit-ports::instrument_mapping tests::fn id
testkit-ports::instrument_mapping tests::candidate_collisions_across_distinct_sources_fail_closed: test-diff:modified:testkit-ports::instrument_mapping tests::candidate_collisions_across_distinct_sources_fail_closed
testkit-ports::instrument_mapping tests::concurrent_distinct_sources_cannot_receive_the_same_candidate: test-diff:modified:testkit-ports::instrument_mapping tests::concurrent_distinct_sources_cannot_receive_the_same_candidate
testkit-ports::instrument_mapping tests::concurrent_issuance_for_same_source_has_one_winner: test-diff:modified:testkit-ports::instrument_mapping tests::concurrent_issuance_for_same_source_has_one_winner
testkit-ports::instrument_mapping tests::concurrent_retirement_never_reactivates_a_source: test-diff:modified:testkit-ports::instrument_mapping tests::concurrent_retirement_never_reactivates_a_source
testkit-ports::instrument_mapping tests::dropping_unpolled_writes_does_not_change_the_mapping: test-diff:modified:testkit-ports::instrument_mapping tests::dropping_unpolled_writes_does_not_change_the_mapping
testkit-ports::instrument_mapping tests::repository_retains_mapping_and_retirement_across_calls: test-diff:modified:testkit-ports::instrument_mapping tests::repository_retains_mapping_and_retirement_across_calls
testkit-ports::instrument_mapping_review tests::repository_reads_retired_history_without_reactivating_or_writing: test-diff:modified:testkit-ports::instrument_mapping_review tests::repository_reads_retired_history_without_reactivating_or_writing
```

## 인계

- **S-21 (`store-sqlite` D-52 구현)**: `InstrumentIdIssuer::issue` 반환 타입(`IssuedInstrumentId`), `InstrumentMappingRepository::ensure` 인자(`IssuedInstrumentId`), `InstrumentMappingStatus::Superseded`, `InstrumentMappingError::Superseded`를 반영하고, 기존 ID 편입과 전역 유일성 제약을 SQLite 스키마·트랜잭션에서 강제한다.
- **T-21 (`broker-toss` 종목 마스터)**: `Superseded` 반환을 `InstrumentMasterEntry::Unavailable`로 유지하고(기존 코드가 이미 모든 매핑 오류를 `Unavailable`로 보존), 향후 사용자 확인 변경·정정 유스케이스와 연결한다.
- **후속 유스케이스 / 화면**: 위 "정의 대기" 표의 정책이 확정되면 `application` 포트 확장, 공시·기업행위 선택 UI, 오기록 정정 흐름을 연결한다.

## 검사 및 손 변형

(검사 결과와 20개 이상 손 변형 표는 실행 후 기록한다.)
