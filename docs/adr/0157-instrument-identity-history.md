# ADR 0157: D-55 확인된 종목 식별자 변경의 이력 연결과 발급원 강제

- 상태: 확정 (2026-11-24)

- 관계: [ADR 0145](0145-instrument-id-mapping.md) 확장 — 기존 D-52 원장 계약에 사용자 확인 기반 식별자 변경 이력 연결, 폐지·연결 오기록 정정, 봉인된 발급 후보 타입을 추가한다. ADR 0145의 "삭제·재활성화 API 없음" 전제는 사용자 확인과 감사 이력을 동반하는 제한적 정정 경로(`correct_retirement`, `correct_link`)로 확장한다.

## 결정

### 1. 확인된 종목 식별자 변경의 이력 연결

종목명·심볼 변경, 액면분할·병합에 따른 ISIN·코드 변경 등 외부 식별자 변경은 `InstrumentMappings::link_confirmed_change`로만 기존 내부 ID(`InstrumentId`)의 이력에 연결한다. 자동 추정이나 브로커 관측·AI 제안만으로는 연결하지 않고 `IdentityUncertain`을 유지한다.

- **확인의 출처(`IdentityConfirmationOrigin`)와 봉인 증명(`UserIdentityConfirmation`)**: `UserConfirmed(UserIdentityConfirmation)`, `BrokerObservation`, `AiSuggestion`을 구분한다. `UserIdentityConfirmation`은 비공개 봉인 필드(`UserIdentityConfirmationSeal`)를 가지며 공개 생성자·`Clone`·`Copy`·`Default`·역직렬화 경로가 없는 일회성 소비 토큰이다([ADR 0039](0039-investor-profile.md) `AdvancedHoursConfirmation`, [ADR 0047](0047-cash-ledger-and-external-flows.md) `UserTransferConfirmation` 선례). `link_confirmed_change`·`correct_retirement`·`correct_link`는 요청을 소유권 이동으로 소비하므로 외부 크레이트(브로커·저장소·AI·MCP 등)에서 `UserConfirmed`를 위조하거나 재사용할 수 없다. 현재 제품 코드에는 발급 어댑터가 없으며 `domain` 모듈 내부 테스트에서만 생성한다. `BrokerObservation`·`AiSuggestion`은 모두 `MappingTransitionError::IdentityUncertain`으로 거부한다.
- **변경 증거(`IdentityChangeEvidence`)**: `None`, `NoteOnly`, `DisclosureOnly`, `ActionOnly`, `DisclosureAndAction`을 구분한다. 현재 보수적 기본값은 공시 키(`DisclosureKey`)·기업행위(`CorporateAction`)·사용자 사유(`UserReason`)가 모두 결합된 `DisclosureAndAction`만 인정하며, 부족한 증거는 `IdentityUncertain`으로 거부한다.
- **기업행위 정합성**:
  - `CorporateAction`의 종목 ID가 변경 대상 내부 ID와 같아야 한다(`InvalidEvidence`).
  - 현재 보수적 기본값으로 허용하는 기업행위는 `SymbolChange`, `Split`, `ReverseSplit` 세 종류뿐이다.
  - `CorporateActionKind::SymbolChange { from, to }`는 ASCII 대소문자 무시 기준으로 `from != to`이고, `from`이 변경 전 `InstrumentSource` 심볼과, `to`가 변경 후 `InstrumentSource` 심볼과 일치해야 한다(`InvalidEvidence`).
  - `CorporateActionKind::Split`·`ReverseSplit`에는 심볼·ISIN 필드가 없으므로 종목 ID가 일치하면 봉인된 사용자 확인·공시·사유와 함께 심볼·ISIN 동시 변경을 포함한 동일 시장 내 비충돌 변경을 허용한다(심볼·ISIN 중 최소 하나 유지를 강제할지는 "정의 대기" 3번으로 올린다).
  - `Delisting`과 `Demerger`(분할 신설은 별개 법인)는 동일 내부 ID 변경 증거로 부적합하므로 `InvalidEvidence`로 거부한다.
  - 아직 동일 ID 승계 정책이 미확정인 `BonusIssue`, `StockDividend`, `Merger`, `Exchange`는 보수적으로 `IdentityUncertain`을 유지한다.
  - 동일 기업행위 재사용 방지, 공시 키-기업행위 교차 검증, 기업행위 일자와 `confirmed_at`·정정 `corrected_at`의 시각 선후·단조성 검증은 현재 미구현이며 "정의 대기" 3·11번으로 올린다.
- **상태 전이와 보존**: 변경이 승인되면 변경 전 `InstrumentSource`는 `InstrumentMappingStatus::Superseded`로 전이하고, 변경 후 `InstrumentSource`는 같은 `InstrumentId`에 `Active`로 추가된다. 이후 변경 전 원본의 `resolve`·`ensure`·`retire`는 모두 `InstrumentMappingError::Superseded`를 반환하며, `lookup`과 `for_id`는 과거 이력을 변경 없이 조회한다.
- **충돌·분기·순환 방지**:
  - `previous`와 `next`가 정확히 같거나 대소문자 무시 기준으로 `(시장, 심볼, ISIN)`이 모두 같으면 `UnchangedSource`, 시장 변경은 `MarketMismatch`.
  - `previous`가 없거나 다른 ID에 묶여 있거나 `Active`가 아니면(`Superseded`/`Retired`) 각각 `Missing`, `SourceBoundToOtherId`, `SourceNotActive`. `previous`가 기존 원본과 심볼/ISIN이 부분 겹침이면 `IdentityUncertain`.
  - `next`가 이미 다른 ID에 묶여 있으면 `SourceBoundToOtherId`, 같은 ID의 기존 이력에 동일 `(시장, 심볼, ISIN)`(대소문자 무시 기준)이 이미 기록되어 있으면 `SourceAlreadyRecorded`, 다른 ID의 기존 원본과 심볼 또는 ISIN이 대소문자 무시 기준으로 겹치면 `IdentityUncertain`.
- **폐지·연결 오기록 정정**:
  - 오기록 정정도 봉인된 `UserConfirmed(UserIdentityConfirmation)` + `MappingCorrectionEvidence::DisclosureAndNote`(공시 키와 사용자 사유)를 요구한다. 부족하면 `IdentityUncertain`으로 거부한다.
  - `correct_retirement`: `Retired` 상태인 해당 ID의 원본을 `Active`로 복구한다(`Active`/`Superseded`는 `SourceNotRetired`).
  - `correct_link`: 보수적 기본값으로 해당 ID의 **가장 최근 활성 연결**(감사 이력 `history`에서 유도한 LIFO 끝단이며 `mistaken_next`가 현재 `Active`)만 되돌린다. 별도의 중복 연결 스택 필드를 두지 않고 `history`의 `IdentifierChanged`·`LinkCorrected`에서 최신 연결을 직접 유도해 내부 상태 불일치 가능성을 제거했다. 되돌리면 `previous`는 `Active`로 복원되고 잘못 연결된 `mistaken_next`는 현재 `records`에서 분리되어(`lookup`은 `Missing`) 이후 독립 발급·재판정이 가능하다. 비최신이거나 `mistaken_next`가 `Retired`이면 `LinkNotLatest`로 거부한다. `records` 분리 대신 별도 `Unlinked` 상태를 남길지 여부는 "정의 대기" 6번으로 올린다.
- **감사 이력(`MappingHistoryEvent`)**: 모든 승인된 변경(`IdentifierChanged`), 폐지 정정(`RetirementCorrected`), 연결 정정(`LinkCorrected`)은 `InstrumentMappings::history` / `history_for_id`에 불변 추가(append-only)로 남긴다. `correct_link`로 레코드가 분리되어도 원래 연결과 정정 이벤트는 감사 이력에서 삭제되지 않는다. 실패한 호출은 원장 상태를 전혀 변경하지 않는다.
- **마스터 투영(`InstrumentMaster`)**: `InstrumentMasterEntry::Unavailable`에 `InstrumentMappingError::Superseded`를 유효한 불가 사유로 추가했다.

| 상황 | 판정 |
| --- | --- |
| 외부 크레이트에서 `UserIdentityConfirmation` 위조·복제 또는 소비된 변경 요청 재사용 | 컴파일 실패(봉인·일회성 소비) |
| `BrokerObservation` 또는 `AiSuggestion` 출처로 변경·정정 요청 | `IdentityUncertain`, 상태 불변 |
| 변경 증거가 `None`/`NoteOnly`/`DisclosureOnly`/`ActionOnly` | `IdentityUncertain`, 상태 불변 |
| 정정 증거가 `None`/`NoteOnly` | `IdentityUncertain`, 상태 불변 |
| 기업행위 종목 ID 불일치, `Delisting`, `Demerger`, `SymbolChange` 심볼 불일치 또는 대소문자만 다른 동일 심볼(`from == to`) | `InvalidEvidence`, 상태 불변 |
| 미확정 기업행위 종류(`BonusIssue`, `StockDividend`, `Merger`, `Exchange`) | `IdentityUncertain`, 상태 불변 |
| 동일 원본 변경(`previous == next`, 대소문자 무시) 또는 시장 변경(`Kr` ↔ `Us`) | `UnchangedSource` / `MarketMismatch`, 상태 불변 |
| `previous`가 `Superseded` 또는 `Retired` 상태에서 변경 연결 | `SourceNotActive`, 상태 불변 |
| `next`가 이미 다른 ID에 묶임 | `SourceBoundToOtherId`, 상태 불변 |
| `next`가 같은 ID의 기존 이력과 정규화 `(시장, 심볼, ISIN)`이 같음(순환 포함) | `SourceAlreadyRecorded`, 상태 불변 |
| `next`가 다른 ID의 기존 원본과 심볼 또는 ISIN이 겹침 | `IdentityUncertain`, 상태 불변 |
| 봉인 사용자 확인 + 공시·허용 기업행위(`SymbolChange`/`Split`/`ReverseSplit`)·사유 충족, 활성 원본에서 비중복 새 원본으로 연결 | 기존 `previous`는 `Superseded`, `next`는 같은 ID의 `Active`, `IdentifierChanged` 이력 추가 |
| 교체된(`Superseded`) 원본의 `resolve`·`ensure`·`retire` | `InstrumentMappingError::Superseded`, 상태 불변 |
| `Retired` 원본의 봉인 사용자 확인 폐지 정정(`DisclosureAndNote`) | 해당 원본 `Active` 복원, `RetirementCorrected` 이력 추가 |
| 가장 최근 활성 연결의 봉인 사용자 확인 연결 정정(`DisclosureAndNote`) | `previous` `Active` 복원, `mistaken_next` 레코드 분리, `LinkCorrected` 이력 추가 |
| 비최신 연결 또는 끝단이 `Retired`인 연결의 정정 요청 | `LinkNotLatest`, 상태 불변 |

### 2. 독립 발급원 타입 강제와 기존 ID 편입·전역 유일성 검증

- **봉인된 후보 타입(`IssuedInstrumentId`)**: 내부 필드가 비공개이고 `Clone`, `Copy`, `Default`, `From<InstrumentId>`와 공개 생성자가 없는 일회성 소유권 토큰이다. 오직 `IndependentInstrumentIdIssuer::issue`만 이 값을 만들 수 있으며, `InstrumentMappings::ensure`와 `InstrumentMappingRepository::ensure`는 후보를 `IssuedInstrumentId` 값으로 소유권을 소비해 받는다.
- **컴파일 시간 차단 증명**: `Symbol`, `IsinCode`, 임의로 생성한 `InstrumentId`를 후보로 전달하거나, `IssuedInstrumentId(...)` 튜플 생성·`try_new`·`From`·`clone`·소비된 후보 재사용·`IndependentInstrumentIdIssuer::clone`, 그리고 `UserIdentityConfirmation` 위조·`default`·`clone`·소비된 `ConfirmedIdentifierChange` 재사용 시도는 모두 컴파일 실패한다(`crates/domain/tests/instrument_issuer_boundaries.rs` 14건, `crates/application/tests/instrument_issuer_boundaries.rs` 4건의 `testkit::check_consumer` 양성 대조 + 음성 프로브로 검증).
- **기존 ID 편입과 예약**:
  - `IndependentInstrumentIdIssuer::from_existing` / `adopt_existing`는 기존 레거시 ID와 이미 발급된 순번 ID를 예약 집합에 편입하고, `issue`는 예약된 ID를 건너뛰며 순번이 `u64::MAX`를 소진하면 `InstrumentIdIssueError::Exhausted`를 반환한다.
  - `InstrumentMappings::adopt_existing_id` / `id_reservation`은 원장 외부에서 이미 존재하는 ID를 예약 집합에 편입해, 미연동 발급원이 같은 ID를 제안해도 `IdAlreadyUsed`로 거부한다.
- **순수 도메인·가짜의 경계와 한계**:
  - `IndependentInstrumentIdIssuer::new()`는 순수 도메인 값 생성자이므로 프로세스 전역 싱글턴을 강제하지 않는다. 전역 유일성은 저장소 직렬화 경계(`MemoryGlobalInstrumentIds` 또는 S-21 SQLite `UNIQUE` 제약 + 기존 ID 편입)가 보장한다.
  - 가짜(`testkit-ports`)의 `MemoryGlobalInstrumentIds`, `MemoryInstrumentIdIssuer`, `MemoryInstrumentMappingRepository`는 공유 전역 레지스트리를 통해 기존 ID 편입, 여러 원장 간·동시 스레드 간 ID 충돌 차단, 폐지된 ID의 타 원장 재발급 차단, 실패한 `ensure`의 후보 미예약, 미폴링(lazy) future의 비변이를 검증한다.
  - 현재 `MemoryGlobalInstrumentIds::ensure_now`는 이미 할당·편입된 집합(`assigned_or_adopted`)과 대조하므로, `global.issuer()`에서 발급되었지만 아직 `ensure`에 도달하지 않은 미적용(in-flight) 후보를 미연동(rogue) 발급원이 선점하는 경우까지는 차단하지 않는다(이 경우 선점된 후보는 여전히 전역 유일하며 후속 충돌 호출은 `IdAlreadyUsed`로 거부된다). S-21에서는 단일 영속 발급원과 저장소 트랜잭션 경계로 통합한다.
  - `IndependentInstrumentIdIssuer`는 현재 내부 순번 상태의 읽기/복원 API를 제공하지 않는다. S-21이 영속 순번 또는 UUIDv7/ULID 기반 발급을 구현할 때 "정의 대기" 10번 결정에 따라 복원/어댑터 경계를 추가해야 한다.

### 3. D-52 포트(`application`) 최소 변경 내역 (S-21 병합 참고)

S-21(`crates/adapters/store-sqlite`)과의 병합 충돌을 최소화하기 위해 `application::instrument_mapping`의 변경을 아래 2개 서명으로 제한했고, 신규 이력 변경·정정 메서드는 포트와 `MemoryInstrumentMappingRepository`에 추측성으로 추가하지 않았다(아래 "정의 대기" 9번 참고).

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
| 1. 확인 출처 증명·권한 경계 (ADR 0145 인계) | `UserIdentityConfirmation` 비공개 봉인(외부 크레이트 생성 불가, 일회성 소비) | A) 전용 사용자 확인 화면·유스케이스 어댑터만 봉인 토큰 발급 / B) 별도 쓰기 포트 + 구조 검사(`arch-check`)로 호출 가능 크레이트를 UI·application으로 제한 | A는 AI·MCP·브로커 어댑터의 위조를 컴파일 단계에서 원천 차단한다(P-9 원칙에 맞춰 확인 화면 자체는 단순하게 유지). B는 어댑터 계층 확장이 쉽지만 `arch-check` 예외 관리가 필요하다. |
| 2. 변경 확인의 최소 증거 | `UserConfirmed` + `DisclosureAndAction`(공시 키·기업행위·사유 모두 필수) | A) 공시+기업행위+사유 필수(현재) / B) 공시+사유 허용(기업행위 미수집 시) / C) 공시 없는 사유만 허용 | A는 오연결 위험이 가장 낮으나 공시·기업행위 수집 전에는 수동 연결이 막힌다. B·C로 완화할수록 공시 지연 시 즉시 처리 가능하나 잘못된 수동 연결 위험이 커진다. |
| 3. 기업행위 종류별 허용 범위·원본 일치·재사용·일자 선후 | `SymbolChange`(`from != to` 대소문자 무시)·`Split`·`ReverseSplit`만 허용(`Split`·`ReverseSplit`은 종목 ID 일치 시 심볼·ISIN 동시 변경 허용), `Delisting`·`Demerger`는 `InvalidEvidence`, `BonusIssue`·`StockDividend`·`Merger`·`Exchange`는 `IdentityUncertain`. 동일 기업행위 재사용·공시 대조·일자 선후는 미검증 | A) 현재 화이트리스트 유지 + 동일 기업행위 재사용 차단·일자 선후(`action.date <= confirmed_at`) 검증 추가 / B) `Split`·`ReverseSplit`에서 심볼·ISIN 동시 변경을 `IdentityUncertain`으로 차단(최소 한 식별자 유지 강제) / C) 합병(`Merger`)·교환(`Exchange`)에 대해 존속 법인 확인 시 조건부 허용 및 공시 키-기업행위 교차 검증 강제 | A는 액면분할로 코드·ISIN이 함께 바뀐 사례를 수용하되 합병 승계는 보류한다. B는 분할 증거 오연결을 더 강하게 막으나 심볼·ISIN 동시 변경 시 별도 절차가 필요하다. C는 합병까지 포괄하나 추가 입력·검증 절차가 늘어난다. |
| 4. 정정 확인의 최소 증거 | `UserConfirmed` + `DisclosureAndNote`(공시 키·사유 필수) | A) 공시+사유 필수(현재) / B) 사용자 사유만으로 오입력 정정 허용 | A는 근거 없는 되돌림을 막지만 사용자가 단순 오타를 직후 정정할 때도 공시 키를 넣어야 한다. B는 오타 복구가 빠르나 감사 근거가 약해진다. |
| 5. 연결 정정 허용 범위 | 해당 ID의 가장 최근 활성 연결(LIFO 끝단)만 정정, 끝단이 폐지 상태면 거부 | A) 최신 활성 연결만 정정(현재) / B) 연결 이후 주문·포지션·평가 참조가 없을 때만 허용 / C) 중간 이력이나 폐지 후 이력도 정정 허용 | A는 이력 그래프가 깨지지 않고 단순하다. B는 실거래 참조 이후 소급 변경을 방지하지만 하위 저장소 교차 검사가 필요하다. C는 과거 오류를 넓게 고칠 수 있으나 손익·원가 이력 정합성이 깨질 위험이 크다. |
| 6. 연결 정정 시 잘못 연결된 원본의 저장 표현 | `records`에서 `mistaken_next` 분리(`lookup`은 `Missing`) + `history`에 불변 감사 이벤트 유지 | A) 레코드 분리 + 감사 이력 유지(현재) / B) `records`에 `Unlinked` 상태로 남겨 `lookup`에서 오연결 이력을 노출하되 별도 ID 발급 허용 | A는 정정된 종목을 즉시 새 종목으로 편입하기 쉽고 `lookup` 의미가 단순하다. B는 `lookup`만으로 과거 오연결 사실을 알 수 있으나 `InstrumentMappingStatus`와 충돌 판정 규칙이 복잡해진다. |
| 7. 동일 심볼 재사용(폐지 후 타 법인 재상장) 처리 | 폐지 이력과 겹치면 `IdentityUncertain` 유지, 자동 재발급 없음 | A) 불확정 유지(현재) / B) 사용자 확인으로 "신규 별개 법인" 판정 후 새 내부 ID 발급 허용 | A는 합병·재상장 혼동을 원천 차단하지만 한국/미국 시장에서 과거 상장폐지 심볼이 새 법인에 재할당될 때 수동 해소가 필요하다. B는 재사용 심볼을 정상 편입할 수 있으나 별도 확인 절차와 화면이 필요하다. |
| 8. 시장 이동·이전 원본 재사용(순환) | 시장 불일치(`MarketMismatch`)와 동일 ID 내 정규화 `(시장, 심볼, ISIN)` 일치 재연결(`SourceAlreadyRecorded`) 거부. 동일 ID에서 과거 심볼을 **새 ISIN**과 함께 다시 쓰는 경우는 현재 차단하지 않음 | A) 현재처럼 `(시장, 심볼, ISIN)` 일치 재연결만 차단 / B) 동일 ID의 과거 심볼 단독 재사용까지 전면 차단 / C) 이전상장(시장 이동)이나 과거 심볼 복귀를 명시적 예외로 허용 | A는 ISIN이 바뀐 재사용을 허용하면서도 동일 원본 순환을 막는다. B는 심볼 복귀 혼동을 더 강하게 차단한다. C는 드문 이전상장·심볼 복귀를 지원하지만 과거 이력 상태 조회 규칙이 복잡해진다. |
| 9. 영속 포트(`InstrumentMappingRepository`) 확장 형태 | 기존 D-52 포트 5개 메서드 유지(S-21 충돌 최소화), 이력 변경·정정은 도메인 우선 구현 | A) `InstrumentMappingRepository`에 변경·정정·기존 ID 편입 메서드 추가 / B) 별도 쓰기 포트(`InstrumentIdentityCorrectionPort`)로 분리 | A는 트랜잭션 경계가 단일 저장소에 모인다. B는 조회·일반 동기화 경로와 사용자 확인 정정 경로의 권한을 타입 수준에서 분리할 수 있다. |
| 10. 독립 발급원 영속 형식과 직렬화 경계 (S-21 선행 조건) | `IndependentInstrumentIdIssuer`는 결정론적 불투명 순번(`inst-{:016x}`) 발급(현재 순번 상태 읽기/복원 API 없음) | A) SQLite 단조 시퀀스 테이블 + `domain`에 시퀀스 복원 생성자 추가 / B) SQLite 어댑터 전용 발급 경계로 UUIDv7·ULID 등 독립 식별자 발급 + DB `UNIQUE` 제약 + 기존 ID 편입 | A는 짧고 결정적이며 테스트 재현성이 높으나 재시작 복원용 시퀀스 저장이 필요하다. B는 다중 원장 병합 시 충돌 가능성이 낮고 순번 복원이 불필요하나 `domain`의 `IssuedInstrumentId` 생성 경계를 별도로 열어야 한다. |
| 11. 감사 이력 시각 선후·단조성 | `confirmed_at`·`corrected_at` 간 시간 순서나 `history` 단조성을 강제하지 않음 | A) `corrected_at >= 대상 사건 시각` 및 ID별 `history` 단조 비감소 강제 / B) 벽시계 보정(NTP 역행)을 고려해 기록 순번(sequence)을 주 순서로 두고 시각 역행은 허용 또는 경고만 표시 | A는 감사 이력의 시각 역전을 원천 차단하지만 기기 시각 보정 직후 사용자가 즉시 정정할 때 거부될 수 있다. B는 시각 보정에도 정정이 막히지 않으나 시각 기준 정렬 대신 순번 기준 정렬에 의존한다. |

## 기존 테스트 기계적 수정 기록

`ensure`의 후보 타입을 `&InstrumentId`에서 소유권 소비형 `IssuedInstrumentId`로 강제하고 `InstrumentIdIssuer::issue` 반환 타입을 `IssuedInstrumentId`로 바꿈에 따라, 아래 기존 테스트 5개 파일의 호출부·준비 코드만 기계적으로 조정했다. 기존 테스트의 단언과 기대 결과는 하나도 삭제하거나 약하게 만들지 않았다.

| 파일 | 변경 대상 테스트 / 헬퍼 | 전 → 후 | 변경 전 SHA-256 (`0b223855`) | 변경 후 SHA-256 |
| --- | --- | --- | --- | --- |
| `crates/domain/tests/instrument_mapping.rs` | 헬퍼 `issued`/`id` 및 기존 7개 테스트의 `ensure` 호출부 | `ensure(&source, &id("..."))` → `ensure(&source, issued("..."))` (`IndependentInstrumentIdIssuer` 결정론 슬롯 발급) | `c5174afbd1b7a02a58ec8488d465aacb5ef8ec92993ba557c2f43415b525d792` | `e7f43fcc8c2c59cc7175a24169cf653faea65086ff178db0c0b2f56e8d68c893` |
| `crates/domain/tests/instrument_mapping_review.rs` | 헬퍼 `issued_slot`/`issued`/`id` 및 기존 3개 테스트의 `ensure` 호출부 | `ensure(&source, &id("..."))` → `ensure(&source, issued("..."))` (상태 기계 모델의 후보 슬롯 일치 유지) | `4939f791d128f65061dccba47610384c340375bee9e80cf9c290487c8c64fb44` | `2d150c00cb908a9f06cb32c7b16829d4699d995b838f830fb6b9b04891fa2e12` |
| `crates/testkit-ports/tests/instrument_mapping.rs` | 헬퍼 `issued_candidate`/`id` 및 기존 6개 테스트의 `ensure` 호출부 | `repo.ensure(&source, &id("..."))` → `repo.ensure(&source, issued_candidate("..."))` | `7d5ac0e0022c06992c76aa4a3e3338767f17ddae8738672d5c47592c4d3fe430` | `19715bcf8ee91efe7094acd8c6ec3346fb8ba5a36eace75bf3c58286970ae205` |
| `crates/testkit-ports/tests/instrument_mapping_review.rs` | `repository_reads_retired_history_without_reactivating_or_writing`의 `ensure` 준비 | `InstrumentId::try_new` 후보 → `IndependentInstrumentIdIssuer::new().issue()` 후보 | `ee6fdb694be65b70dd03956508e5c472fe6bfca38c8aae2ba795a0718b1d1aeb` | `84a85fa20066f574b76cc06bf01c738732216f728df338aca1b3bf34d6492628` |
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

- **S-21 (`store-sqlite` D-52 구현) 및 후속 영속 포트 확장**: `InstrumentIdIssuer::issue` 반환 타입(`IssuedInstrumentId`), `InstrumentMappingRepository::ensure` 인자(`IssuedInstrumentId`), `InstrumentMappingStatus::Superseded`, `InstrumentMappingError::Superseded`를 반영하고, 기존 ID 편입과 전역 유일성 제약을 SQLite 스키마·트랜잭션에서 강제한다. 재시작 후 영속 발급원 구현을 위해서는 "정의 대기" 10번(시퀀스 복원 API 추가 vs UUIDv7/ULID 어댑터 발급 경계) 결정이 선행되어야 하며, "정의 대기" 9번에 따라 이력 변경·정정을 영속 저장할 때는 `records`·`reserved_ids`·`history`(연결 LIFO 스택 유도 근거)를 하나의 원자적 트랜잭션으로 저장·복원하고 불일치 시 복원을 거부해야 한다.
- **T-21 (`broker-toss` 종목 마스터)**: `Superseded` 반환을 `InstrumentMasterEntry::Unavailable`로 유지하고(기존 코드가 이미 모든 매핑 오류를 `Unavailable`로 보존), 향후 사용자 확인 변경·정정 유스케이스와 연결한다.
- **후속 유스케이스 / 화면**: "정의 대기" 1~9번 정책이 확정되면 `UserIdentityConfirmation` 발급 어댑터(확인 화면·유스케이스 경계), `application` 포트 확장, 공시·기업행위 선택 UI, 오기록 정정 흐름을 연결한다.

## 검사 및 손 변형

### 범위 검사 요약

- `cargo fmt --all -- --check`: 통과
- `cargo clippy --locked --all-targets -p domain -p application -p testkit-ports -p broker-toss -- -D warnings`: 경고 0건
- 테스트:
  - `domain` 내부 이력·정정·상태 기계(`instrument_mapping_change::history_cases::tests`): 15건 통과(결정론 경로 + 독립 참조 모델 속성 테스트 포함)
  - `domain` 외부 경계·편입·유일성·마스터(`instrument_identity_history`): 4건 통과
  - `domain` 컴파일 실패 경계(`instrument_issuer_boundaries`): 14개 프로브 통과(양성 1 + 음성 13: `Symbol`/`IsinCode`/임의 `InstrumentId` 전달, `IssuedInstrumentId` 위조·복제·재사용, 발급원 복제, `UserIdentityConfirmation` 위조·`Default`·복제, 소비된 `ConfirmedIdentifierChange` 재사용)
  - `domain` 기존 매핑 회귀(`instrument_mapping` 7건, `instrument_mapping_review` 3건, `t21_instrument_master` 6건): 전부 통과
  - `application` 포트 컴파일 실패 경계(`instrument_issuer_boundaries`): 4개 프로브 통과(양성 1 + 음성 3)
  - `testkit-ports` 가짜·전역 유일성·동시성(`instrument_mapping` 6건, `instrument_mapping_d55` 7건, `instrument_mapping_review` 1건): 전부 통과
  - `broker-toss` 종목 마스터 회귀(`t21_instruments`): 35건 통과
- 커버리지(`cargo llvm-cov` on `domain`):
  - `crates/domain/src/instrument_id.rs`: 줄 42/42 (100.00%), 영역 59/59 (100.00%)
  - `crates/domain/src/instrument_issuer.rs`: 줄 72/72 (100.00%), 영역 109/109 (100.00%)
  - `crates/domain/src/instrument_mapping.rs`: 줄 331/331 (100.00%), 영역 424/424 (100.00%)
  - `crates/domain/src/instrument_mapping_change.rs`: 줄 224/224 (100.00%), 영역 196/196 (100.00%)
  - `crates/domain/src/instrument_mapping_change/history_cases.rs`: 줄 1079/1079 (100.00%), 영역 2054/2054 (100.00%)
  - `crates/domain/src/instrument_master.rs`: 줄 40/40 (100.00%), 영역 65/65 (100.00%)
- 정적·하네스 검사: `arch-check` 위반 0건, `source-check` 위반 없음, `raw-types` 위반 없음, `disable-check` 위반 없음, `disable-report-since 0b223855` 새 끄기 0건(Rust 0, TS 0), `hooks-installed` 통과, `just deny` 통과(`advisories/bans/licenses/sources ok`), `codex-hooks/config.mjs`·`claude-hooks/hashes.mjs`·`source-tree/check.mjs`·`config-files/check.mjs` 모두 위반 0건.
- `just test-diff 0b2238554479d46b69be508c4375d2d09b0ea8e4`: 위 기계적 수정 30건 외 추가 위반 없음.

### 손 변형 (격리 복사본 `target/D-55/mutants-work` 실측: 30/30 컴파일·검출, 생존 0)

| 번호 | 대상 파일 | 변형 | 결과 |
| --- | --- | --- | --- |
| 1 | `domain/src/instrument_issuer.rs` | 발급 순번 증가량을 1에서 2로 변경 | 검출: `independent_issuer_and_ledger_enforce_existing_id_adoption` |
| 2 | `domain/src/instrument_issuer.rs` | 발급원 `adopt_existing`의 예약 삽입·중복 검사 제거 | 검출: `independent_issuer_and_ledger_enforce_existing_id_adoption`, `issuer_never_reemits_adopted_or_previously_issued_ids` |
| 3 | `domain/src/instrument_issuer.rs` | 발급원 `reservation` 조회 반전 | 검출: `independent_issuer_and_ledger_enforce_existing_id_adoption`, `issuer_never_reemits_adopted_or_previously_issued_ids` |
| 4 | `domain/src/instrument_issuer.rs` | 발급원 `issue`의 기존 예약 ID 건너뛰기 제거 | 검출: `independent_issuer_and_ledger_enforce_existing_id_adoption`, `issuer_never_reemits_adopted_or_previously_issued_ids` |
| 5 | `domain/src/instrument_mapping_change.rs` | 식별자 변경에서 `BrokerObservation` 출처 허용 | 검출: `unconfirmed_origins_stay_identity_uncertain`, `history_and_correction_state_machine_matches_independent_model` |
| 6 | `domain/src/instrument_mapping_change.rs` | 식별자 변경에서 `AiSuggestion` 출처 허용 | 검출: `unconfirmed_origins_stay_identity_uncertain` |
| 7 | `domain/src/instrument_mapping_change.rs` | 변경 전후 동일 원본 검사를 대소문자 구분 비교로 약화 | 검출: `identifier_change_handles_ascii_case_invariants_without_bypassing_cycle_guard` |
| 8 | `domain/src/instrument_mapping_change.rs` | 변경 전후 시장 불일치(`MarketMismatch`) 검사 제거 | 검출: `identifier_change_rejects_invalid_shape_and_action_evidence`, `history_and_correction_state_machine_matches_independent_model` |
| 9 | `domain/src/instrument_mapping_change.rs` | 기업행위 종목 ID와 변경 대상 내부 ID 일치 검사 제거 | 검출: `identifier_change_rejects_invalid_shape_and_action_evidence` |
| 10 | `domain/src/instrument_mapping_change.rs` | `SymbolChange` 대소문자만 다른 동일 심볼(`from_norm == to_norm`) 차단 제거 | 검출: `identifier_change_rejects_case_only_symbol_change_and_undecided_actions` |
| 11 | `domain/src/instrument_mapping_change.rs` | `SymbolChange` 이전 심볼 일치 검사 제거 | 검출: `identifier_change_rejects_invalid_shape_and_action_evidence` |
| 12 | `domain/src/instrument_mapping_change.rs` | `SymbolChange` 이후 심볼 일치 검사 제거 | 검출: `identifier_change_rejects_invalid_shape_and_action_evidence` |
| 13 | `domain/src/instrument_mapping_change.rs` | 상장폐지(`Delisting`) 기업행위를 식별자 변경 증거로 허용 | 검출: `identifier_change_rejects_invalid_shape_and_action_evidence` |
| 14 | `domain/src/instrument_mapping_change.rs` | 분할 신설(`Demerger`) 기업행위를 동일 ID 변경 증거로 허용 | 검출: `identifier_change_rejects_invalid_shape_and_action_evidence` |
| 15 | `domain/src/instrument_mapping_change.rs` | 미확정 합병(`Merger`) 기업행위를 `IdentityUncertain` 대신 허용 | 검출: `identifier_change_rejects_case_only_symbol_change_and_undecided_actions` |
| 16 | `domain/src/instrument_mapping_change.rs` | 미확정 무상증자·주식배당·교환 기업행위를 허용 | 검출: `identifier_change_rejects_case_only_symbol_change_and_undecided_actions` |
| 17 | `domain/src/instrument_mapping_change.rs` | 정정 검증에서 `BrokerObservation` 출처 허용 | 검출: `retirement_and_link_corrections_reject_unconfirmed_origin_or_missing_evidence` |
| 18 | `domain/src/instrument_mapping_change.rs` | 정정 검증에서 `AiSuggestion` 출처 허용 | 검출: `retirement_and_link_corrections_reject_unconfirmed_origin_or_missing_evidence` |
| 19 | `domain/src/instrument_mapping.rs` | `Superseded` 원본의 `retire` 허용 | 검출: `confirmed_isin_change_supersedes_previous_and_keeps_internal_id`, `history_and_correction_state_machine_matches_independent_model` |
| 20 | `domain/src/instrument_mapping.rs` | `check_next_target`의 타 ID/동일 ID 충돌 오류 분기 반전 | 검출: `identifier_change_rejects_branching_cycles_and_retired_sources`, `identifier_change_rejects_missing_and_cross_id_conflicts` |
| 21 | `domain/src/instrument_mapping.rs` | 동일 ID 내 대소문자 변형 순환 연결 차단 제거 | 검출: `identifier_change_handles_ascii_case_invariants_without_bypassing_cycle_guard` |
| 22 | `domain/src/instrument_mapping.rs` | 변경 후 원본의 타 ID 부분 겹침(`IdentityUncertain`) 검사 제거 | 검출: `identifier_change_rejects_missing_and_cross_id_conflicts`, `history_and_correction_state_machine_matches_independent_model` |
| 23 | `domain/src/instrument_mapping.rs` | 원장 `adopt_existing_id`의 예약 집합 편입 제거 | 검출: `independent_issuer_and_ledger_enforce_existing_id_adoption` |
| 24 | `domain/src/instrument_mapping.rs` | 확정 변경 시 이전 원본의 `Superseded` 상태 전이 누락 | 검출: `confirmed_isin_change_supersedes_previous_and_keeps_internal_id`, `chained_changes_preserve_ordered_records_and_audit_history` |
| 25 | `domain/src/instrument_mapping.rs` | `correct_retirement`의 비폐지 원본 거부 제거 | 검출: `retirement_correction_validates_owner_state_and_restores_active`, `link_correction_rejects_non_latest_wrong_owner_and_retired_tip` |
| 26 | `domain/src/instrument_mapping.rs` | `correct_link`의 최신 연결(LIFO) 제한 제거 | 검출: `link_correction_rejects_non_latest_wrong_owner_and_retired_tip` |
| 27 | `domain/src/instrument_mapping.rs` | `correct_link`의 끝단 `Active` 상태 검사 제거(`Retired` 허용) | 검출: `link_correction_rejects_non_latest_wrong_owner_and_retired_tip` |
| 28 | `testkit-ports/src/memory.rs` | 가짜 저장소 `ensure`의 전역 ID 유일성 검사 제거 | 검출: `global_registry_adopts_existing_ids_and_prevents_cross_ledger_collisions`, `retired_id_stays_reserved_globally_while_next_issued_id_succeeds`, `concurrent_cross_ledger_writes_preserve_global_uniqueness` |
| 29 | `domain/src/instrument_mapping.rs` | `latest_active_link`에서 `LinkCorrected` 스택 팝 제거 | 검출: `link_correction_unwinds_latest_link_and_releases_mistaken_source` |
| 30 | `testkit-ports/src/memory.rs` | 가짜 저장소 `ensure_now`의 전역 발급원 기존 ID 편입 제거 | 검출: `dropping_unpolled_issuer_and_repository_futures_preserves_state` |
