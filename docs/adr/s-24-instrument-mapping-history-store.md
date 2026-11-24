# ADR 초안: S-24 종목 식별자 변경 이력·정정의 영속 (1단계 + 2단계)

- 상태: 초안 (2026-11-24)
- 결정한 사람: 구현 담당 초안(1단계·2단계 설계 결정 메인 확정 반영, 최종 병합 확인 대기)
- 관련 작업: S-24 (선행 D-55, S-21, H-64; 연관 T-21, S-15, S-16)
- 반영한 문서: `crates/domain/src/disclosure.rs`, `crates/domain/src/instrument_mapping.rs`, `crates/domain/src/instrument_mapping_change.rs`, `crates/domain/src/lib.rs`, `crates/application/src/instrument_mapping.rs`, `crates/application/src/lib.rs`, `crates/testkit-ports/src/memory.rs`, `crates/testkit-ports/src/contracts.rs`, `crates/testkit-ports/src/contracts/instrument_mapping.rs`, `crates/testkit-ports/src/contracts/instrument_mapping_correction.rs`, `crates/adapters/store-sqlite/migrations/0009_instrument_mapping_history.sql`, `crates/adapters/store-sqlite/migrations.lock`, `crates/adapters/store-sqlite/schema.snapshot.sql`, `crates/adapters/store-sqlite/src/instrument_mapping.rs`, `crates/adapters/store-sqlite/src/sql/instrument_mapping.rs`

## 맥락

D-55([docs/adr/0157-instrument-identity-history.md](0157-instrument-identity-history.md))는 사용자 확인(`UserIdentityConfirmation`)과 증거를 동반한 종목 식별자 변경(`IdentifierChanged`), 폐지 정정(`RetirementCorrected`), 연결 정정(`LinkCorrected`) 및 `Superseded` 승계 쌍 규칙을 도메인에 정의했다. S-21([docs/adr/0158-instrument-mapping-store.md](0158-instrument-mapping-store.md))은 1:1 `active`/`retired` 대응 원장과 독립 ID 발급원(`instrument_id_issuer`)을 `0008_instrument_mappings.sql`로 영속화하면서, 복수 원본(`Superseded` 승계 쌍)·감사 이력·정정 영속은 **S-24**로 인계했다.

S-24는 D-55의 `UserIdentityConfirmation` 봉인 경계와 맞물려 있으므로 메인 결정(선택지 A + C)에 따라 3단으로 진행하되, 마이그레이션 `0009`를 한 번만 고정하기 위해 S-24 1단계와 2단계를 같은 브랜치(`s-24`)에서 완결한 뒤 한 번에 병합한다.

- **H-64 (별도 병합 완료, [docs/adr/0162-identity-confirmation-issuance.md](0162-identity-confirmation-issuance.md))**: `UserIdentityConfirmation` 발급 경계(`ConfirmInstrumentIdentity`, `IdentityConfirmationPrompt`, 보호된 시험용 `ScriptedIdentityConfirmation`·`ScriptedIdentityConfirmationIssuer`, `arch-check` 호출 위치 강제)를 구축했다.
- **S-24 1단계 (완료)**:
  1. 마이그레이션 `0009_instrument_mapping_history.sql` 및 `0008` 자료 무손실 이관.
  2. `domain`의 검증 전용 사건 재생·원장 대조 함수 `verify_persisted_mapping_ledger` 추가(`InstrumentMappings`나 확인 토큰을 돌려주지 않음), `DisclosureKey::from_receipt(ReceiptNo)` 최소 생성자 추가, `CorporateAction` 첨부 거래(`receipt()`) 거부 강화.
  3. `Database::open`에서 스키마·전체 트리거·사건 이력 재생↔원장 대조·`superseded_event_seq` 대응을 검증하고 어긋나면 열기 실패(`RepositoryIntegrity`).
  4. 읽기 전용 이력 포트 `InstrumentMappingHistoryRepository` 분리 및 가짜·SQLite 공유 계약 테스트.
  5. 사건 직렬화 형식 확정 및 왕복·깨진 값 거부 테스트.
- **S-24 2단계 (이 문서에 합산 완료)**:
  1. 별도 쓰기 포트 `application::InstrumentMappingCorrectionRepository`(`link_confirmed_change`, `correct_retirement`, `correct_link`)와 전용 오류 열거형 `MappingTransitionStoreError { Transition(MappingTransitionError), Storage(SystemPortError) }` 신설(메인 결정 1-A).
  2. `domain`에 소유된 `Confirmed*` 값과 현재 관련 원장/이력 부분집합을 검증해 기록할 `MappingHistoryEvent`를 산출하는 순수 계획 함수 `plan_confirmed_identifier_change`, `plan_confirmed_retirement_correction`, `plan_confirmed_link_correction`을 두고 `InstrumentMappings`와 `store-sqlite`가 단일 규칙 원천을 공유(비봉인 `InstrumentMappings` 생성자 미노출).
  3. SQLite `InstrumentMappingStore`의 `InstrumentMappingCorrectionRepository` 구현: `BEGIN IMMEDIATE` 단일 트랜잭션에서 관련 행·이력 조회, 도메인 전이 검증, 사건 추가, 원장 갱신(행 영향 수 `1` 검증)을 원자적으로 수행하고 중간 실패 시 완전 롤백, 두 연결 경쟁 시 직렬화 및 잠금 시간 초과 시 `Unavailable` 반환.
  4. `testkit-ports`의 `MemoryInstrumentMappingStore` 래퍼 및 가짜·SQLite 공유 쓰기 계약(충돌·정정 규칙 전수, 미폴링 드롭 무변경, 두 연결 경쟁 9종 — `ensure`/`retire`/연결/정정 조합, 거짓 통과 방지 `#[should_panic]` 4종) + `application` 포트 경계 단일 소비·미확인 프롬프트 차단 컴파일 실패 프로브 7종.
  5. 1단계 인계 6항목 전부 해소(도메인 메모리↔SQLite 상태 기계 차등 테스트 및 폐지 정정 후 재폐지·미확인 재활성화 거부 회귀 테스트 포함).

## 스키마와 제약 (`0009_instrument_mapping_history.sql`)

마이그레이션 `0009_instrument_mapping_history.sql`(SHA-256 `b6e60a6119333d85641ea8073b26f04673105104d97493a36a40be319f8a25fb`)은 기존 `0008`의 `instrument_mappings`를 무손실 이관하면서 감사 사건 표와 확장된 원장 표를 구성한다. 기존 `0008_instrument_mappings.sql`과 `instrument_id_issuer`는 변경하지 않는다.

| 대상 | 종류 | 정의 | 역할 |
|---|---|---|---|
| `instrument_mapping_events` | `STRICT` 테이블 | `seq INTEGER PRIMARY KEY CHECK (seq > 0)`, `event_kind`, `instrument_id`, `market`, `prev_symbol`, `prev_isin`, `next_symbol`, `next_isin`, `disclosure_source`, `disclosure_receipt_no`, `action_kind`, `action_year`, `action_month`, `action_day`, `action_from_symbol`, `action_to_symbol`, `action_from_count`, `action_to_count`, `note`, `recorded_seconds`, `recorded_nanos`, `recorded_offset` | 감사 사건 추가 전용 원장. `Split`/`ReverseSplit` 주수 열은 `MONEY_SEGMENTS` 회피를 위해 `action_from_count`/`action_to_count`(앞자리 `0` 금지 양의 10진 `TEXT`, 최대 20자리)를 사용하며 `u64::MAX`까지 무손실 표현 |
| 사건 테이블 열·행 `CHECK` | 복합 제약 | 도메인 문법(`InstrumentId`·`KnownMarket`·`Symbol`·`IsinCode`·`DisclosureKey` DART/KIND 14자리 숫자·SEC EDGAR `10-2-6`·`UserReason` `length(CAST(note AS BLOB)) BETWEEN 1 AND 512` 및 `length(CAST(trim(note, ' \t\n\r') AS BLOB)) > 0` — SQLite `length(TEXT)`의 NUL 절단을 피해 앞자리·단독·중간 NUL 및 512바이트 경계 포함 도메인 유효 `UserReason`을 무손실 보존·`Timestamp` 초/나노초/분단위 오프셋) + `event_kind`별 필수/NULL 열 일치 + `link_corrected`·`identifier_changed` 대소문자 무시 변경 검증 + `SymbolChange`/`Split`/`ReverseSplit` 변형별 열 및 비율 방향 정합 | 깨진 사건 직렬화 행 삽입을 DB 계층에서 1차 차단(유니코드 공백·달력 일자 유효성·오프셋 적용 연도 범위·`u64::MAX` 초과는 Rust 디코더와 `domain`이 닫힘 검증) |
| `instrument_mapping_events_instrument_seq` | `INDEX` | `(instrument_id, seq)` | `history_for_id` 및 트리거의 `max(seq) WHERE instrument_id = ...` 점조회(풀스캔 0건) |
| `instrument_mapping_events_no_update` / `no_delete` | `BEFORE UPDATE` / `BEFORE DELETE` 트리거 | 모든 수정·삭제에 `RAISE(ABORT, ...)` | 감사 사건 추가 전용 불변성 강제 |
| `instrument_mapping_events_monotonic_seq` | `BEFORE INSERT` 트리거 | `NEW.seq != coalesce((SELECT max(seq) FROM instrument_mapping_events), 0) + 1` 시 `RAISE(ABORT, ...)` | `1..=N` 공백 없는 단조 순번 강제(`INSERT OR REPLACE` 포함 차단) |
| `instrument_mappings` (재생성) | `STRICT` 테이블 | `PRIMARY KEY (instrument_id, market, symbol, isin)`, `UNIQUE (market, symbol, isin)`, `status IN ('active', 'superseded', 'retired')`, `superseded_event_seq INTEGER REFERENCES instrument_mapping_events(seq)`, `CHECK ((status = 'superseded' AND superseded_event_seq IS NOT NULL) OR (status IN ('active', 'retired') AND superseded_event_seq IS NULL))` | 동일 `InstrumentId`의 `Superseded` 승계 쌍 저장 허용 및 고아 `superseded` 행 DB 차원 차단. `ALTER TABLE ... RENAME TO instrument_mappings_legacy_0008` → `INSERT ... SELECT ..., NULL` → `DROP TABLE`로 `0008` 활성·폐지 행 무손실 이관 |
| `instrument_mappings_symbol_key` / `isin_key` | 비고유 `INDEX` | `(market, upper(symbol))`, `(upper(isin))` | 기존 인덱스 이름 유지(`BY_SOURCE`, `BY_SOURCE_OR_ID`, 트리거 점조회 성능 보존), 동일 ID 승계 쌍의 심볼 또는 ISIN 유지 허용 |
| `instrument_mappings_active_retired_{id,symbol,isin}_key` | 부분 `UNIQUE INDEX` | `WHERE status IN ('active', 'retired')` | ID당 종단 행(`active`/`retired`) 최대 1건, 활성·폐지 심볼/ISIN 대소문자 무시 전역 고유성 강제 |
| `instrument_mappings_superseded_event_key` | 부분 `UNIQUE INDEX` | `(superseded_event_seq) WHERE superseded_event_seq IS NOT NULL` | 각 `IdentifierChanged` 사건 `seq`가 원장의 `superseded` 행 최대 1건에만 결합되도록 DB 차원 1:1 고유성 강제 |
| `instrument_mappings_no_delete` | `BEFORE DELETE` 트리거 | `OLD.status != 'active'` 또는 해당 ID 최신 사건이 일치하는 `link_corrected`가 아니면 `RAISE(ABORT, ...)` | 일반 삭제 전면 차단, 최신 `link_corrected`의 오연결 활성 행 분리만 허용 |
| `instrument_mappings_retire_only` | `BEFORE UPDATE` 트리거 | 식별 열 변경 금지; `active→retired`, 최신 `identifier_changed` 일치 `active→superseded(seq)`, 최신 `retirement_corrected` 일치 `retired→active`, 최신 `link_corrected` 일치 `superseded→active(NULL)` 4경로만 허용 | 사건 없는 상태 전이·재활성화·과거 사건 `seq` 재사용 차단(단 D-55 `MappingHistoryEvent`가 일반 `retire`를 사건으로 기록하지 않으므로, `retirement_corrected` 이후 재폐지된 동일 원본의 원시 SQL 재활성화 차단은 2단계 애플리케이션 쓰기 경계가 담당) |
| `instrument_mappings_no_replace` | `BEFORE INSERT` 트리거 | 타 ID와 심볼/ISIN 겹침, 동일 ID의 기존 `active`/`retired`·시장 불일치·정규화 중복, 그리고 기존 ID에 새 행 추가 시 최신 `identifier_changed` 부재를 모두 차단 | 타 ID가 `Superseded` 심볼/ISIN을 침범하거나 `INSERT OR REPLACE`로 이력을 덮어쓰는 경로 차단 |
| `instrument_mappings_insert_active_only` | `BEFORE INSERT` 트리거 | `NEW.status != 'active' OR NEW.superseded_event_seq IS NOT NULL` 시 `RAISE(ABORT, ...)` | 비활성·`superseded` 직접 삽입 우회 차단 |

## 상황 → 결과

| 연산 | 상황 | 결과 | 비고 |
|---|---|---|---|
| `Database::open` | 빈 DB, `0008` 활성·폐지 DB 자동 승격, 또는 `0009` 정상 승계·정정 이력 + 원장 DB | `Ok(Database)` | `validate_on_open`: 매핑·이력·발급원 DDL 20종 + DB 전체 트리거 16종 대조 + `verify_persisted_mapping_ledger` + `verify_superseded_event_links` + `validate_issuer_cursor` |
| `Database::open` | 순번 공백(`1..=N` 위반), 깨진 사건 열(미지 종류·공시·유니코드 공백 사유·달력 무효일·`u64::MAX` 초과 주수·범위 밖 나노초/오프셋), 사건 재생과 원장 불일치, `superseded_event_seq` 오연결/취소 사건 참조, 고아 `superseded`, 또는 `0009` 인덱스·트리거 변조 | `Err(SystemPortError { port: Repository, failure: RepositoryIntegrity })` | 닫힘 실패. 감사 시각(`recorded_seconds`/`nanos`/`offset`) 단조성은 검증하지 않음(ADR 0157 메인 결정 11번: `seq`가 주 순서) |
| `InstrumentMappingHistoryRepository::history()` | 일반 `ensure`/`retire`만 수행된 DB | `Ok(vec![])` | 변경·정정 감사 사건만 기록 |
| `InstrumentMappingHistoryRepository::history()` | 정상 변경·정정 사건이 있는 DB(재시작 포함) | `Ok(Vec<MappingHistoryEvent>)` | `seq` 오름차순으로 사건 종류·대소문자·공시 출처/번호·기업행위·사유·시각(초/나노초/오프셋) 무손실 반환 |
| `InstrumentMappingHistoryRepository::history_for_id(id)` | 사건 있는 ID / 사건 없는 등록 ID / 미등록 ID | 해당 ID 사건(`seq` 순) 또는 `Ok(vec![])` | 존재하지 않는 ID도 빈 벡터 반환 |
| `InstrumentMappingRepository::{resolve,ensure,retire}` | `Superseded` 상태 원본 | `Err(MappingStoreError::Mapping(Superseded))` | |
| `InstrumentMappingRepository::{lookup,for_id}` | `Superseded` 원본 및 동일 ID 복수 행 | `lookup`: `Superseded` 레코드 반환 / `for_id`: 해당 ID의 `Superseded` + `Active`/`Retired` 전체 반환 | `LinkCorrected`로 분리된 오연결 원본은 `Missing`이 되어 새 발급 가능; `Superseded` 원본의 심볼·ISIN은 타 ID 겹침 시 `IdentityUncertain` 유지 |
| `InstrumentMappingCorrectionRepository::link_confirmed_change(change)` | `UserConfirmed` + 공시·적합 기업행위·사유, `previous` 활성 소유, `next` 미점유·같은 시장·타 ID 겹침 없음 | `Ok(instrument_id)` | 단일 `BEGIN IMMEDIATE` 트랜잭션에서 `INSERT event(identifier_changed)` → `UPDATE previous SET status='superseded', superseded_event_seq=seq` → `INSERT next('active')`. 재시작 `Database::open` 검증 통과 |
| `InstrumentMappingCorrectionRepository::link_confirmed_change(change)` | 비사용자 출처·불완전 증거·첨부 거래 포함·미등록·타 ID 소유·비활성(`Superseded`/`Retired`)·동일 원본·시장 불일치·기등록(`SourceAlreadyRecorded`)·타 ID 심볼/ISIN 부분 겹침(`IdentityUncertain`) | `Err(MappingTransitionStoreError::Transition(...))` | 사건·원장 무변경. 중간 트리거/제약 실패 또는 손상 원장/순번 한계는 `Err(Storage(RepositoryIntegrity))`와 완전 롤백, 잠금 시간 초과는 `Err(Storage(Unavailable))` |
| `InstrumentMappingCorrectionRepository::correct_retirement(correction)` | `UserConfirmed` + 공시·사유, 해당 ID 소유의 `Retired` 원본 | `Ok(instrument_id)` | 단일 `BEGIN IMMEDIATE` 트랜잭션에서 `INSERT event(retirement_corrected)` → `UPDATE source SET status='active'`. 정정 후 재폐지된 원본은 `ensure`와 미확인 재정정이 거부되고, 새 `UserConfirmed` 정정 시 새 `seq`로 재기록 |
| `InstrumentMappingCorrectionRepository::correct_link(correction)` | `UserConfirmed` + 공시·사유, 해당 ID의 LIFO 최상단 미취소 `IdentifierChanged(previous → mistaken_next)`이며 `previous`가 `Superseded`이고 `mistaken_next`가 `Active` | `Ok(instrument_id)` | 단일 `BEGIN IMMEDIATE` 트랜잭션에서 `INSERT event(link_corrected)` → `DELETE mistaken_next` → `UPDATE previous SET status='active', superseded_event_seq=NULL WHERE superseded_event_seq = latest_seq`. `superseded_event_seq` 드리프트 시 `RepositoryIntegrity` 롤백. 2단계 체인의 역순(LIFO) 해제 및 완전 분리 원본의 새 ID 재발급·동일 쌍 재연결 허용 |
| `InstrumentMappingCorrectionRepository` 미폴링 Future 드롭 / 두 연결 동시 경쟁 | 쓰기 Future를 폴링하지 않고 드롭 / 두 연결이 동일 이전 원본·타 ID 동일 대상·타 ID 부분 겹침·연결 대 폐지·연결 대 `ensure`(동일 대상·부분 겹침)·이중 폐지 정정·이중 연결 정정·연결 정정 대 후속 연결 9종으로 동시 경쟁 | 미폴링은 사건·원장 무변경 / 동시 경쟁은 `BEGIN IMMEDIATE` 직렬화로 정확히 1건만 선점하고 패자는 도메인 충돌 오류(또는 `ensure`의 `Existing`)로 수렴 | 모든 경쟁 종료 후 `Database::open` 재생 검증 통과 |
| `domain::verify_persisted_mapping_ledger(records, history)` | `InstrumentMappings`가 생성 가능한 모든 합법적 원장·이력 상태(비단조 감사 시각 포함) | `Ok(())` | 검증 전용(상태·토큰 미반환). 불일치·증거 위반·첨부 거래 포함·타 ID 겹침·고아 `Superseded`는 `Err(MappingLedgerIntegrityError)` |
| `domain::InstrumentMappings::link_confirmed_change` | `CorporateAction::with_receipt(...)` 첨부 거래가 있는 `Split`/`ReverseSplit`/`SymbolChange` | `Err(MappingTransitionError::InvalidEvidence)` | 영속 직렬화에서 첨부 거래가 조용히 유실되는 것을 막기 위해 도메인 검증 시점에 닫힘 거부 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| **읽기 전용 이력 포트·쓰기 정정 포트 분리 (정의 대기 1 → 선택지 A 확정)**: 기존 `InstrumentMappingRepository`는 변경하지 않고 `application::InstrumentMappingHistoryRepository`(`history`, `history_for_id`)와 `application::InstrumentMappingCorrectionRepository`(`link_confirmed_change`, `correct_retirement`, `correct_link`)를 별도 트레이트로 분리한다. 쓰기 3연산은 도메인 소유 타입(`ConfirmedIdentifierChange`, `ConfirmedRetirementCorrection`, `ConfirmedLinkCorrection`)을 값으로 직접 소비하고 `MappingTransitionStoreError { Transition(MappingTransitionError), Storage(SystemPortError) }`를 반환한다 | 일반 조회·동기화(`broker-toss`, `T-21`), 감사 이력 조회, 사용자 확인 쓰기 정정의 의존 경계를 분리한다. 소유된 `Confirmed*` 타입(`Copy`/`Clone` 없음, 내부 `UserIdentityConfirmationSeal` 봉인)을 포트 메서드가 값으로 소비하므로 컴파일 시점에 미확인 프롬프트 전달(`E0308`)과 확인 값 재사용(`E0382`)이 원천 차단된다. 또한 `MappingTransitionError`를 유지해 호출부가 `SourceNotActive`/`LinkNotLatest`/`IdentityUncertain` 등 실패 원인을 정확히 구분할 수 있다 | `InstrumentMappingRepository`에 이력·정정 메서드를 직접 추가하는 안(모든 기존 구현·가짜·T-21 목에 정정 책임 강제); `application` 전용 입력 DTO로 이중 래핑하는 안(확인 봉인 중복) | 메인 S-24 1·2단계 결정(1-A), `docs/adr/0157-instrument-identity-history.md`, `docs/adr/0162-identity-confirmation-issuance.md` |
| **도메인 순수 계획 함수(`plan_confirmed_*`)로 규칙 단일 원천화**: `domain`에 `plan_confirmed_identifier_change`, `plan_confirmed_retirement_correction`, `plan_confirmed_link_correction`을 추가하고 `InstrumentMappings`와 `store-sqlite`가 같은 내부 검증·사건 생성 로직을 호출한다 | `store-sqlite`는 `InstrumentMappings` 비봉인 생성자를 열지 않으면서도(D-55 봉인 유지) `BEGIN IMMEDIATE` 트랜잭션 안에서 조회한 관련 원장·이력 부분집합과 소유 `Confirmed*` 값을 `domain`에 넘겨 도메인 전이 규칙을 100% 동일하게 판정받고 기록할 `MappingHistoryEvent`를 돌려받는다. 규칙 중복과 드리프트를 구조적으로 없앤다 | `store-sqlite`가 도메인 전이 규칙을 SQL/Rust로 별도 재구현하는 안(규칙 드리프트 위험); `InstrumentMappings::restore`를 여는 안(D-55 봉인 파괴) | `docs/adr/0157-instrument-identity-history.md`, `docs/adr/0158-instrument-mapping-store.md` |
| **`MemoryInstrumentMappingStore` 래퍼 도입 및 `MemoryInstrumentMappingHistoryStore` 분리 유지 (인계 4항목 통합 검토 결론)**: `testkit-ports`에서 기존 `MemoryInstrumentMappingRepository`의 동기 `link_confirmed_change`/`correct_retirement`/`correct_link`(H-64 `identity_confirmation_boundary` 테스트 사용)를 그대로 유지하고, `&MemoryInstrumentMappingRepository`를 감싼 `MemoryInstrumentMappingStore<'repo>`가 `InstrumentMappingRepository` + `InstrumentMappingHistoryRepository` + `InstrumentMappingCorrectionRepository`(지연 `poll_fn` 실행) 3포트를 모두 구현한다. 1단계의 `MemoryInstrumentMappingHistoryStore::try_from_verified`는 별도 읽기 전용 스냅샷 가짜로 유지한다 | H-64 보호 경계 테스트(`identity_confirmation_boundary.rs`)를 1줄도 바꾸지 않고 `clippy::same_name_method` 충돌 없이 가짜와 SQLite가 동일한 비동기 쓰기 공유 계약(`unpolled` 드롭·충돌·동시 경쟁 9종)을 실행한다. `MemoryInstrumentMappingHistoryStore`는 확인 토큰 없이 사전 검증된 원장·이력 스냅샷(`try_from_verified`)을 시드해 1단계 읽기 계약과 불일치 시드 거부를 시험하는 불변 어댑터이므로, 가변 쓰기 저장소(`MemoryInstrumentMappingStore`)와 합치면 D-55 확인 경계와 불변 시드 검증 책임이 섞인다 | `MemoryInstrumentMappingRepository`의 동기 메서드를 제거하거나 이름을 바꾸는 안(H-64 테스트 수정 필요); `MemoryInstrumentMappingHistoryStore`에 비봉인 시드 + 쓰기를 합치는 안(확인 토큰 없는 가변 원장 경로 생성) | H-64 경계 보존, `crates/testkit-ports/src/memory.rs` |
| **검증 전용 도메인 API(`verify_persisted_mapping_ledger`)로 D-55 봉인 보존**: 비봉인 `&[InstrumentMappingRecord]`·`&[MappingHistoryEvent]`를 받아 규칙대로 재생·대조한 뒤 `Result<(), MappingLedgerIntegrityError>`만 반환한다. `InstrumentMappings`나 `UserIdentityConfirmation`을 절대 돌려주지 않는다 | `InstrumentMappings` 복원 생성자를 열면 외부 크레이트가 `IssuedInstrumentId`·`UserIdentityConfirmation` 봉인을 거치지 않고 임의의 원장·이력을 조립해 도메인 메서드를 호출할 수 있게 되어 D-55 봉인이 깨진다(S-21 D-55 맞춤 리뷰 [1] 선례). 저장소는 `verify_persisted_mapping_ledger`로 원장↔사건 일관성만 검증하고, 실제 쓰기 상태 전이는 H-64 `UserIdentityConfirmation`을 소비하는 2단계 쓰기 포트가 원자적 트랜잭션으로 수행한다 | `InstrumentMappings::restore_with_history(...)`로 도메인 원장 인스턴스를 반환하는 안(D-55 봉인 약화); `UserIdentityConfirmation` 공개 생성자를 여는 안(H-64 분리 결정 위반) | 메인 결정(A + C), `docs/adr/0157-instrument-identity-history.md`, `docs/adr/0158-instrument-mapping-store.md` |
| **`Database::open` 전체 사건 재생·대조 유지 (정의 대기 2 → 선택지 A 확정)**: 열기 시 전체 `instrument_mapping_events` 재생·원장 대조와 `superseded_event_seq` 1:1 대조를 유지한다 | 개인용 앱의 종목 식별자 변경·정정 건수는 평생 수십~수백 건 수준으로 `PERF-1`(2초)에 미치는 영향이 미미하며, 체크포인트 스냅샷의 복잡도와 불일치 위험을 피한다 | 열기 검증용 체크포인트 스냅샷 도입(불필요한 복잡도) | 메인 S-24 2단계 결정(2-A) |
| **로컬 열기 검증 범위 및 S-16 인계 (정의 대기 3 → 선택지 A + S-16 인계 확정)**: 열기 시점 스키마·전체 트리거·사건 재생·`superseded_event_seq` 대조를 제공하고, DB 키와 파일 쓰기 권한을 가진 오프라인 동시 재작성 탐지는 S-16 키체인 고정 MAC 체인으로 인계한다 | 로컬 무결성 닫힘 검증과 키체인 기반 변조 탐지 책임을 명확히 분리한다 | S-24에서 별도 임시 MAC 구현(S-16과 중복) | 메인 S-24 2단계 결정(3-A + S-16 인계) |
| **`0009` 단순 트리거 유지 + 2단계 쓰기 경계·회귀 테스트로 재폐지 후 재활성화 차단 (정의 대기 4 → 선택지 A 확정)**: `0009` 스키마·트리거는 1단계 확정본(`b6e60a61...`)을 변경 없이 유지하고, `retirement_corrected` 후 재폐지된 원본의 미확인 재활성화는 `InstrumentMappingCorrectionRepository::correct_retirement` 경계(새 `UserIdentityConfirmation` 필수 + 새 `seq` 사건 삽입)와 공유 계약·차등 회귀 테스트로 차단한다 | 일반 `retire`가 감사 사건이 아닌 D-55 구조에서 `0009`에 1회 소비 보조 열·표를 추가하는 스키마 복잡도를 피한다. 원시 SQL 직접 변조는 오프라인 파일 변조 위협 모델(S-16)로 일관 귀속한다 | `0009`에 `retirement_corrected` 1회 소비 보조 열·표 추가 | 메인 S-24 2단계 결정(4-A) |
| **`DisclosureKey::from_receipt(ReceiptNo)` 최소 생성자 추가**: `Disclosure` 본문·URL 없이 검증된 `ReceiptNo` 하나만으로 `DisclosureKey`를 복원한다 | ADR 0024의 핵심 불변식은 "`DisclosureKey`는 검증된 `ReceiptNo`를 봉인하며 열거형 판별자가 `source`를 결정하므로, 검증된 번호와 어긋난 출처를 전달할 수 없다"이다. `from_receipt(ReceiptNo)`는 `Dart(DartReceiptNo)`·`Kind(KindReceiptNo)`·`SecEdgar(SecEdgarReceiptNo)`의 사전 검증 타입만 받으므로 출처·번호 불일치를 원천 차단하며, 감사 사건 직렬화 복원 시 `CollectedItem`/`Url` 위조 없이 공시 키만 정확히 복원할 수 있다 | 저장소 복원용으로 가짜 URL·제목을 붙여 `Disclosure::try_new`를 호출하는 안(가짜 `CollectedItem` 생성으로 ADR 0024 취지 훼손); `DisclosureKey::from_raw(source, str)`를 여는 안(출처-번호 불일치 허용) | 메인 세부 결정, `docs/adr/0024-collected-information-types.md` |
| **`CorporateAction` 첨부 거래(`receipt()`) 거부**: `validate_change_action`에서 `action.receipt().is_some()`이면 `MappingTransitionError::InvalidEvidence`로 거부한다 | 식별자 변경 증거로 허용되는 `SymbolChange`·`Split`·`ReverseSplit`에는 원래 영수 거래(`Transaction`)가 없으나, `CorporateAction::with_receipt`로 첨부할 수 있었다. 사건 표에 첨부 거래를 저장하지 않으면 영속 왕복 시 조용히 유실되므로 도메인 검증 단계에서 닫힘 거부해 왕복 동등성을 보장한다 | 사건 표에 `Transaction` 직렬화 열을 추가하는 안(식별자 변경 증거에 불필요한 거래 스키마 중복); 영속 시 첨부 거래를 조용히 버리는 안(손실 직렬화) | 메인 세부 결정 |
| **`Split`/`ReverseSplit` 비율 열 이름으로 `action_from_count`/`action_to_count` 사용 및 정밀 양의 10진 `TEXT` 저장**: `ShareCount(u64)` 전 범위(`1..=u64::MAX`)를 앞자리 `0` 없는 10진 문자열로 저장한다 | SQLite `INTEGER`는 `i64::MAX`를 넘는 `u64` 값을 담지 못하며, 열 이름에 `ratio`·`shares` 대신 `count`를 사용해 `sql-check`의 금액 열(`MONEY_SEGMENTS`) 오탐과 무관한 제약을 피한다 | `INTEGER` 열 사용(`u64::MAX` 손실); `action_old_shares`/`action_ratio` 등 모호한 이름 사용 | 메인 세부 결정, `crates/xtask/src/sql_check/` |
| **감사 사건 시각(`recorded_seconds`, `recorded_nanos`, `recorded_offset`) 비단조 허용 및 무손실 보존**: 3개 `INTEGER` 열로 초·나노초·오프셋을 정확히 복원하되, 사건 간 시각 단조성은 검증하지 않는다 | ADR 0157 메인 결정 11번에 따라 NTP 시각 보정 등 벽시계 역행 상황에서도 정정이 막히지 않도록 단조 순번 `seq`를 유일한 주 순서로 삼는다 | `recorded_seconds >= prev.recorded_seconds` 트리거/검증 강제(ADR 0157 결정 11번 위배) | `docs/adr/0157-instrument-identity-history.md` 11번 |
| **`MigrationSchemaTracker`로 `0001..=0009` 누적 DDL 추적**: `ALTER TABLE ... RENAME TO`와 `DROP TABLE`을 반영해 현재 스키마 객체 집합을 계산하고 `sqlite_schema`와 대조한다 | `0009`가 `instrument_mappings`를 재생성하면서 `0008`의 이전 테이블·인덱스·트리거가 교체된다. 단순 누적 수집은 교체된 `0008` 객체를 중복 기대하게 되므로 `RENAME`/`DROP`을 반영해 최종 20개 매핑·이력·발급원 객체와 전체 16개 트리거를 정확히 검증한다. `MigrationSchemaTracker`는 현행 마이그레이션의 단일 줄 `CREATE TRIGGER`·명시 `ALTER TABLE ... RENAME TO`·`DROP TABLE` 형식을 전제하며, 어긋나면 `Database::open` 테스트에서 즉시 닫힘 실패한다 | `0009` 객체만 하드코딩 목록으로 대조하는 안(마이그레이션 원천과 이중 관리되어 드리프트 위험) | `docs/adr/0158-instrument-mapping-store.md` |

## 1단계 정의 대기 항목 확정 결과

| 번호 | 항목 | 확정 선택지 | 반영 위치 |
|---|---|---|---|
| 1 | 2단계 쓰기 포트 서명·오류 열거형 | **선택지 A** (`InstrumentMappingCorrectionRepository` 별도 분리 + 도메인 소유 `Confirmed*` 직접 소비 + `MappingTransitionStoreError`) | `crates/application/src/instrument_mapping.rs`, `crates/adapters/store-sqlite/src/instrument_mapping.rs` |
| 2 | `Database::open` 검증 비용 상한 | **선택지 A** (현행 전체 재생·대조 유지) | `crates/adapters/store-sqlite/src/instrument_mapping.rs` |
| 3 | 로컬 오프라인 변조 위협 모델 한계 | **선택지 A + S-16 인계** (로컬 열기 검증 + S-16 키체인 MAC 체인 인계) | 본 ADR 및 S-16 인계 |
| 4 | `retirement_corrected` 후 재폐지 행의 재활성화 차단 위치 | **선택지 A** (`0009` 단순 트리거 유지 + 2단계 쓰기 경계·회귀 테스트로 차단, 원시 SQL은 S-16 범위) | `crates/testkit-ports/src/contracts/instrument_mapping_correction.rs`, `crates/adapters/store-sqlite/src/instrument_mapping_correction_tests.rs` |

## 1단계 인계 6항목 해소 내역

1. **`application` 쓰기 포트 신설**: `application::InstrumentMappingCorrectionRepository`와 `MappingTransitionStoreError` 구현 및 `application` 공개 재수출 완료.
2. **SQLite 쓰기 구현**: `InstrumentMappingStore::link_confirmed_change`/`correct_retirement`/`correct_link`를 `BEGIN IMMEDIATE` 단일 트랜잭션과 `domain::plan_confirmed_*` 단일 규칙 원천으로 구현 완료(`0009` 스키마 변경 0줄).
3. **쓰기 원자성·중간 실패·두 연결 경쟁·잠금 시간 초과 테스트**: 중간 트리거 중단 5경로 롤백, `superseded_event_seq` 드리프트 롤백, 고아 `superseded` 원장 및 `i64::MAX`/`0` 순번 거부, 두 연결 동시 경쟁 9종(연결·폐지·`ensure` 동일/부분 겹침·정정 조합) + 재열기, `BEGIN IMMEDIATE` 잠금 대기 직렬화, `ZERO_BUSY_TIMEOUT` 3연산 `Unavailable` 검증 완료.
4. **가짜·SQLite 쓰기 공유 계약(`testkit-ports`)**: `MemoryInstrumentMappingStore` 래퍼 도입(`MemoryInstrumentMappingHistoryStore` 불변 스냅샷 읽기 가짜와 책임 분리 유지 근거는 위 결정 표 참조) 및 `contracts::instrument_mapping_correction` 공유 계약(생명주기·충돌 규칙, 미폴링 드롭 무변경, 두 연결 경쟁 9종) + 거짓 통과 방지 `#[should_panic]` 4종 + `application` 포트 단일 소비·미확인 프롬프트 컴파일 실패 프로브 7종 구현 완료.
5. **도메인 메모리↔SQLite 상태 기계 차등 테스트**: `domain_memory_and_sqlite_match_across_state_machine_differential_sequences`에서 `MemoryInstrumentMappingStore`와 SQLite `InstrumentMappingStore`를 매 스텝(`ensure`/`retire`/`link_confirmed_change`/`correct_retirement`/`correct_link`, 미확인·불완전 증거·대소문자 혼합·NUL 사유·비단조 시각 포함) 대조하고 재열기 동등성 및 통계 하한(`ok_ops`, `err_ops`, `id_changed`, `retire_corr`, `link_corr`, `re_retire_re_corrected`, `max_chain_len >= 2`) 검증 완료.
6. **폐지 정정 후 재폐지·재활성화 회귀 테스트**: `verify_re_retire_and_second_correction` 공유 계약(메모리 + SQLite + 재열기)과 차등 테스트 `re_retire_re_corrected > 0`에서 `correct_retirement` → `retire` → `ensure` 거부 → 미확인 재정정 거부 → 새 `UserConfirmed` 재정정 시 새 `seq` 기록까지 검증 완료.

## 후속 인계

- **S-16**: `instrument_mapping_events` 머리값 및 원장 무결성을 키체인 고정 HMAC 체인에 결합해 DB 키 + 오프라인 파일 쓰기 권한 보유자의 동시 재작성 탐지.
- **S-15 / T-21 후속 UI·유스케이스**: 사용자 확인 화면을 `ConfirmInstrumentIdentity` → `InstrumentMappingCorrectionRepository`로 연결.

## 기존 테스트 변경 (기계적 고정 개수 갱신)

마이그레이션 `0009` 추가에 따라 마이그레이션 원장 개수와 `sqlite_master` 스키마 객체 개수를 고정 상수로 단언하는 기존 테스트 2건과 미래 마이그레이션 상수 2줄만 기계적으로 갱신했다(`crates/adapters/store-sqlite/src/instrument_mapping_tests.rs`는 0줄 변경, 기존 단언 삭제·약화 0건).

> 참고: 메인 지시대로 아래 2건의 승인 행은 구조 보존 규칙(`for`·`?`·클로저) 때문에 `approvals.toml`로 해소되지 않을 수 있으며, 메인이 병합 커밋에서 정확히 이 2건만 지적되는지 확인한다.

| 파일 | 변경 내용 | 기준 SHA-256 (`6b8cabf3`) | 변경 후 SHA-256 | `test-diff-fast` 지적 문자열 |
|---|---|---|---|---|
| `crates/adapters/store-sqlite/src/migration_tests.rs` | 마이그레이션 원장 개수 단언 `8` → `9` (1줄) | `d62952637a9c9e99a314ba9e8d229b89295351668e5c26d8004fd29f7160df8f` | `240cc894071421b0f04205cfba0dce04c67f4ff0880c5db44623c77330911a75` | `store-sqlite migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger: test-diff:modified:store-sqlite migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger` |
| `crates/adapters/store-sqlite/src/tests.rs` | `sqlite_master` 스키마 객체 개수 단언 `55` → `64` (+9 = 이벤트 테이블 1 + 이벤트 명시 인덱스 1 + 이벤트 트리거 3 + 원장 부분 고유 인덱스 4, 1줄) | `1f82f383ed3a04d89cb8059535fcf6676146399c89db050429dea9fdf1ba30e0` | `306405fff3ab503eeaaec63e626300deb913f81321040c382e19e33b3f02ef50` | `store-sqlite tests::raw_key_is_independently_readable_and_debug_is_closed: test-diff:modified:store-sqlite tests::raw_key_is_independently_readable_and_debug_is_closed` |
| `crates/adapters/store-sqlite/src/sql/holdings_tests.rs` | 동반 픽스처 상수 `GAP`/`FUTURE` `9` → `10` (2줄) | `aef1e733b1fa7b6852683064161ffb23488577cd9f3ff035748d81b0ac6a19ba` | `0dfb84a1310ca9a507be1868ec2c77a137b7fa2fb13313ce91341286e2560986` | 위반 없음 |
| `crates/adapters/store-sqlite/src/sql/instrument_mapping_tests.rs` | `0009` 무제약 테이블·트리거 복구 픽스처 SQL 2상수 동기화 (`instrument_mapping_tests.rs` 본체는 0줄 변경) | `9ace0df125e525f5db440271c52cb8e72fcd6d468a20bd48bda1926d7610f359` | `cf6600ff809fabe71baeabf106e1442cbf5ef5a035d9f7abb56a945b3626d158` | 위반 없음 |

제안 `harness/test-diff/approvals.toml` 행(구조 보존 규칙으로 해소되지 않을 수 있음):

```toml
[[approval]]
task = "S-24"
path = "crates/adapters/store-sqlite/src/migration_tests.rs"
before_sha256 = "d62952637a9c9e99a314ba9e8d229b89295351668e5c26d8004fd29f7160df8f"
after_sha256 = "240cc894071421b0f04205cfba0dce04c67f4ff0880c5db44623c77330911a75"
finding = "test-diff:modified:store-sqlite migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger"
decision = "approved"
reason = "마이그레이션 0009_instrument_mapping_history.sql 추가에 따라 원장 개수 기대값을 8에서 9로 갱신"
assertion_change = true

[[approval]]
task = "S-24"
path = "crates/adapters/store-sqlite/src/tests.rs"
before_sha256 = "1f82f383ed3a04d89cb8059535fcf6676146399c89db050429dea9fdf1ba30e0"
after_sha256 = "306405fff3ab503eeaaec63e626300deb913f81321040c382e19e33b3f02ef50"
finding = "test-diff:modified:store-sqlite tests::raw_key_is_independently_readable_and_debug_is_closed"
decision = "approved"
reason = "마이그레이션 0009 추가(+9: 이벤트 테이블 1, 이벤트 인덱스 1, 이벤트 트리거 3, 원장 부분 고유 인덱스 4)에 따라 스키마 객체 기대값을 55에서 64로 갱신"
assertion_change = true
```

## 실측 및 손 변형

> 아래 표는 1단계 완료(`4c13d306` 기준) 실측치다. 2단계 합산(`6b8cabf3` 기준) 커버리지·손 변형·자동 변경분 변이(`just mutants-daily 6b8cabf3... 0/1`)는 메인 지시에 따라 2단계 리뷰가 모두 끝난 뒤 1회 수행해 아래에 합산 갱신한다.

### 1단계 정적 검사·테스트·커버리지·자동 변이 (`4c13d306ed6190f2fbd970822a92ddf88f1a7529` 기준)

| 항목 | 실측 결과 |
|---|---|
| 정적 검사 (`just check-fast-static`) | 17종 전 항목 통과(`deny`·`gen-schema`·`fmt`·`arch-check`·`source-check`·`sql-check`·`sqlcipher`·`migration-check`·`capability-check`·`raw-types`·`disable-check`·`source-tree`·`config-files`·`claude-hooks/hashes`·`codex-hooks/config`·`workflow-policy`·`review-records` 위반 0건) |
| 신규 검사 끄기 (`disable-report-fast`) | `4c13d306ed6190f2fbd970822a92ddf88f1a7529 이후 새 끄기 0건 (Rust 0건, TS 0건)` |
| Clippy (`--all-targets -- -D warnings`) | `domain`·`application`·`testkit-ports`·`store-sqlite` 경고 0건 |
| 테스트 (`cargo nextest run --locked`) | `domain`·`application`·`testkit-ports`·`store-sqlite` 전체 통과 |
| 기존 테스트 변경 (`test-diff-fast`) | 위 표의 기계적 고정 개수 갱신 2건(`migration_tests.rs` `8→9`, `tests.rs` `55→64`)만 보고, 그 외 0건 |
| 커버리지 (`cargo llvm-cov`) | `domain/src/disclosure.rs` 영역 237/237(100.00%)·함수 20/20(100.00%)·줄 165/165(100.00%); `domain/src/instrument_mapping.rs` 영역 816/816(100.00%)·함수 64/64(100.00%)·줄 617/617(100.00%); `domain/src/instrument_mapping_change.rs` 영역 198/198(100.00%)·함수 28/28(100.00%)·줄 224/224(100.00%); `adapters/store-sqlite/src/instrument_mapping.rs` 함수 98/98(100.00%)·줄 892/896(99.55%)·영역 1443/1526(94.56%) |
| 자동 변경분 변이 (`just mutants-daily 4c13d306ed6190f2fbd970822a92ddf88f1a7529 0/1`) | `mutants: 선택 변이 158개` / `158 mutants tested in 51m: 120 caught, 38 unviable` / `mutants: 위반 없음` (생존 0건, 시간 초과 0건) |
| 손 변형 (`target/S-24/work`) | 격리 사본에서 27건(M01~M27) 실제 컴파일·실행, **27/27 검출(생존 0건)** 후 `target/S-24` 삭제 확인 |

### 손 변형 목록 (M01~M27, 격리 사본 실측)

| 번호 | 대상 파일 | 변형 내용 | 결과 및 검출 테스트 |
|---|---|---|---|
| M01 | `crates/domain/src/instrument_mapping_change.rs` | `validate_change_action`에서 `action.receipt().is_some()` 거부 제거 | 검출 (`domain::instrument_mapping_history_verify::tests::verify_rejects_invalid_event_evidence_and_attached_receipt`) |
| M02 | `crates/domain/src/disclosure.rs` | `DisclosureKey::source`를 `DisclosureSource::Dart`로 고정 | 검출 (`domain::instrument_mapping_history_verify::tests::disclosure_key_from_receipt_preserves_source_and_receipt_number`) |
| M03 | `crates/domain/src/instrument_mapping.rs` | `apply_identifier_changed`에서 `previous.market() != next.market()` 검사 제거 | 검출 (`domain::instrument_mapping_history_verify::tests::verify_rejects_invalid_event_evidence_and_attached_receipt`) |
| M04 | `crates/domain/src/instrument_mapping.rs` | `apply_identifier_changed`에서 `superseded_contains_normalized(next)` 거부 제거 | 검출 (`domain::instrument_mapping_history_verify::tests::verify_rejects_unlinked_and_branching_transitions_within_same_id`) |
| M05 | `crates/domain/src/instrument_mapping.rs` | `ReplaySourceIndex::remove`에서 심볼 `EntryPresence::Occupied` 유지 조건 제거 | 검출 (`domain::instrument_mapping_history_verify::tests::verify_keeps_shared_symbol_owner_across_link_correction`) |
| M06 | `crates/domain/src/instrument_mapping.rs` | `ReplaySourceIndex::remove`에서 ISIN `EntryPresence::Occupied` 유지 조건 제거 | 검출 (`domain::instrument_mapping_history_verify::tests::verify_keeps_shared_isin_owner_across_link_correction`) |
| M07 | `crates/domain/src/instrument_mapping.rs` | `apply_link_corrected`에서 `live_index.remove(unlinked_next)` 생략 | 검출 (`domain::instrument_mapping_history_verify::tests::verify_frees_unlinked_isin_for_other_id`) |
| M08 | `crates/domain/src/instrument_mapping.rs` | `apply_retirement_corrected`에서 `live_index.insert(instrument_id, source)` 생략 | 검출 (`domain::instrument_mapping_history_verify::tests::verify_rejects_cross_id_previous_and_retirement_index_conflicts`) |
| M09 | `crates/domain/src/instrument_mapping.rs` | `verify_persisted_mapping_ledger`에서 이력 없는 ID의 고아 `Superseded` 검사 제거 | 검출 (`domain::instrument_mapping_history_verify::tests::verify_rejects_terminal_and_market_violations_in_stored_records`) |
| M10 | `crates/adapters/store-sqlite/src/instrument_mapping.rs` | `validate_ledger`에서 `verify_superseded_event_links` 호출 생략 | 검출 (`store-sqlite::instrument_mapping_history_tests::open_rejects_mismatched_superseded_event_seq_and_unbacked_superseded_rows`) |
| M11 | `crates/adapters/store-sqlite/src/instrument_mapping.rs` | `validate_ledger`에서 `verify_persisted_mapping_ledger` 호출 생략 | 검출 (`store-sqlite::instrument_mapping_history_tests::open_rejects_active_tip_mismatch_against_retirement_corrected_event`) |
| M12 | `crates/adapters/store-sqlite/src/instrument_mapping.rs` | `load_all_events`에서 `entry.seq != expected_seq` 연속성 검사 생략 | 검출 (`store-sqlite::instrument_mapping_history_tests::open_and_history_reads_reject_tampered_event_seq_kind_and_action_columns`) |
| M13 | `crates/adapters/store-sqlite/src/instrument_mapping.rs` | `load_events_for_id`에서 `entry.seq <= previous_seq` 단조 검사 생략 | 검출 (`store-sqlite::instrument_mapping_history_tests::history_for_id_rejects_duplicate_seq_in_unconstrained_events`) |
| M14 | `crates/adapters/store-sqlite/src/instrument_mapping.rs` | `decode_status`에서 `seq > 0`을 `seq >= 0`으로 완화 | 검출 (`store-sqlite::instrument_mapping_history_tests::decoder_rejects_zero_superseded_seq`) |
| M15 | `crates/adapters/store-sqlite/src/instrument_mapping.rs` | `decode_share_count`에서 `raw.starts_with('0')` 거부 제거 | 검출 (`store-sqlite::instrument_mapping_history_tests::open_and_history_reads_reject_tampered_event_seq_kind_and_action_columns`) |
| M16 | `crates/adapters/store-sqlite/src/instrument_mapping.rs` | `decode_share_count`에서 ASCII 숫자 전용 검사 제거(`+1` 허용) | 검출 (`store-sqlite::instrument_mapping_history_tests::open_and_history_reads_reject_cross_kind_action_columns_and_plus_share_count`) |
| M17 | `crates/adapters/store-sqlite/src/instrument_mapping.rs` | `decode_event_body` `link_corrected`에서 `reject_unchanged_source` 생략 | 검출 (`store-sqlite::instrument_mapping_history_tests::open_and_history_reads_reject_cross_kind_action_columns_and_plus_share_count`) |
| M18 | `crates/adapters/store-sqlite/src/instrument_mapping.rs` | `validate_records`에서 `RequireEveryIdTerminal`의 `*count != 1` 검사 제거 | 검출 (`store-sqlite::instrument_mapping_history_tests::for_id_rejects_corrupt_unconstrained_records_for_same_id`) |
| M19 | `crates/adapters/store-sqlite/src/instrument_mapping.rs` | `MigrationSchemaTracker::apply`에서 `ALTER TABLE ... RENAME TO` 처리 생략 | 검출 (`store-sqlite::instrument_mapping_history_tests::migration_schema_tracker_tracks_rename_and_drop_of_tables_indexes_and_triggers`) |
| M20 | `crates/adapters/store-sqlite/src/instrument_mapping.rs` | `MigrationSchemaTracker::mapping_objects`에서 `instrument_mapping_events` 누락 | 검출 (`store-sqlite::instrument_mapping_history_tests::open_rejects_tampered_0009_schema_objects`) |
| M21 | `migrations/0009_instrument_mapping_history.sql` + `src/sql/instrument_mapping.rs` | `instrument_mapping_events_monotonic_seq` 조건을 `NEW.seq <= 0`으로 약화 | 검출 (`store-sqlite::instrument_mapping_history_tests::event_table_triggers_enforce_append_only_and_monotonic_seq`) |
| M22 | `migrations/0009_instrument_mapping_history.sql` + `src/sql/instrument_mapping.rs` | `Split` 비율 방향 `CHECK`(`action_to_count > action_from_count`) 제거 | 검출 (`store-sqlite::instrument_mapping_history_tests::event_table_checks_reject_malformed_event_rows`) |
| M23 | `crates/testkit-ports/src/memory.rs` | `MemoryInstrumentMappingHistoryStore::try_from_verified`에서 `verify_persisted_mapping_ledger` 생략 | 검출 (`testkit-ports::instrument_mapping_history_contract::tests::memory_history_store_passes_shared_populated_history_contract_and_rejects_inconsistent_seed`) |
| M24 | `crates/testkit-ports/src/memory.rs` | `MemoryInstrumentMappingHistoryStore::history_for_id`에서 ID 필터 제거 | 검출 (`testkit-ports::instrument_mapping_history_contract::tests::memory_history_store_passes_shared_populated_history_contract_and_rejects_inconsistent_seed`) |
| M25 | `crates/adapters/store-sqlite/src/instrument_mapping.rs` | `RecordSetValidator::observe`에서 `*counter > 1`을 `*counter < 1`로 변경 | 검출 (`store-sqlite::instrument_mapping_history_tests::lookup_rejects_multiple_terminal_records_for_same_id_in_partial_slice`) |
| M26 | `migrations/0009_instrument_mapping_history.sql` + `src/sql/instrument_mapping.rs` | `note` CHECK의 `length(CAST(trim(...) AS BLOB)) > 0`을 `length(trim(...)) > 0`으로 되돌려 앞자리 NUL `UserReason` 거부 | 검출 (`store-sqlite::instrument_mapping_history_tests::history_roundtrips_user_reason_containing_nul_byte`) |
| M27 | `migrations/0009_instrument_mapping_history.sql` + `src/sql/instrument_mapping.rs` | `note` CHECK 상한을 `BETWEEN 1 AND 512`에서 `BETWEEN 1 AND 511`로 축소 | 검출 (`store-sqlite::instrument_mapping_history_tests::history_roundtrips_512_byte_boundary_user_reasons`) |
