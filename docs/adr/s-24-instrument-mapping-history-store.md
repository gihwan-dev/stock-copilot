# ADR 초안: S-24 종목 식별자 변경 이력·정정의 영속 (1단계)

- 상태: 초안 (2026-11-24)
- 결정한 사람: 구현 담당 초안(사용자·메인 확인 대기)
- 관련 작업: S-24 (선행 D-55, S-21; 연관 H-64, T-21, S-15, S-16)
- 반영한 문서: `crates/domain/src/disclosure.rs`, `crates/domain/src/instrument_mapping.rs`, `crates/domain/src/instrument_mapping_change.rs`, `crates/application/src/instrument_mapping.rs`, `crates/testkit-ports/src/memory.rs`, `crates/testkit-ports/src/contracts/instrument_mapping.rs`, `crates/adapters/store-sqlite/migrations/0009_instrument_mapping_history.sql`, `crates/adapters/store-sqlite/migrations.lock`, `crates/adapters/store-sqlite/schema.snapshot.sql`

## 맥락

D-55([docs/adr/0157-instrument-identity-history.md](0157-instrument-identity-history.md))는 사용자 확인(`UserIdentityConfirmation`)과 증거를 동반한 종목 식별자 변경(`IdentifierChanged`), 폐지 정정(`RetirementCorrected`), 연결 정정(`LinkCorrected`) 및 `Superseded` 승계 쌍 규칙을 도메인에 정의했다. S-21([docs/adr/0158-instrument-mapping-store.md](0158-instrument-mapping-store.md))은 1:1 `active`/`retired` 대응 원장과 독립 ID 발급원(`instrument_id_issuer`)을 `0008_instrument_mappings.sql`로 영속화하면서, 복수 원본(`Superseded` 승계 쌍)·감사 이력·정정 영속은 **S-24**로 인계했다.

S-24는 D-55의 `UserIdentityConfirmation` 봉인 경계와 맞물려 있으므로 메인 결정(선택지 A + C)에 따라 작업을 둘로 나눈다.

- **H-64 (별도 작업)**: `UserIdentityConfirmation` 발급 경계(`UserPresence` 선례: 단일 발급 함수 + `arch-check`로 호출 위치를 확인 유스케이스와 보호된 시험용 발급 가짜로 제한)를 구축한다.
- **S-24 1단계 (이 문서의 범위)**:
  1. 마이그레이션 `0009_instrument_mapping_history.sql` 및 `0008` 자료 무손실 이관.
  2. `domain`의 검증 전용 사건 재생·원장 대조 함수 `verify_persisted_mapping_ledger` 추가(`InstrumentMappings`나 확인 토큰을 돌려주지 않음), `DisclosureKey::from_receipt(ReceiptNo)` 최소 생성자 추가, `CorporateAction` 첨부 거래(`receipt()`) 거부 강화.
  3. `Database::open`에서 스키마·전체 트리거·사건 이력 재생↔원장 대조·`superseded_event_seq` 대응을 검증하고 어긋나면 열기 실패(`RepositoryIntegrity`).
  4. 읽기 전용 이력 포트 `InstrumentMappingHistoryRepository` 분리 및 가짜·SQLite 공유 계약 테스트.
  5. 2단계 쓰기가 그대로 재사용할 사건 직렬화 형식 확정 및 왕복·깨진 값 거부 테스트(SQL 픽스처).
- **S-24 2단계 (H-64 착륙 후 후속)**: 쓰기 3연산 포트(`link_confirmed_change`, `correct_retirement`, `correct_link`), `BEGIN IMMEDIATE` 쓰기 원자성·중간 실패·두 연결 경쟁, 쓰기 공유 계약(아래 "2단계 인계" 참조).

## 스키마와 제약 (`0009_instrument_mapping_history.sql`)

마이그레이션 `0009_instrument_mapping_history.sql`(SHA-256 `875b31c198e0f425755dd6a4d2ea8edf2d9819acaf0f4dc494142dad4401a639`)은 기존 `0008`의 `instrument_mappings`를 무손실 이관하면서 감사 사건 표와 확장된 원장 표를 구성한다. 기존 `0008_instrument_mappings.sql`과 `instrument_id_issuer`는 변경하지 않는다.

| 대상 | 종류 | 정의 | 역할 |
|---|---|---|---|
| `instrument_mapping_events` | `STRICT` 테이블 | `seq INTEGER PRIMARY KEY CHECK (seq > 0)`, `event_kind`, `instrument_id`, `market`, `prev_symbol`, `prev_isin`, `next_symbol`, `next_isin`, `disclosure_source`, `disclosure_receipt_no`, `action_kind`, `action_year`, `action_month`, `action_day`, `action_from_symbol`, `action_to_symbol`, `action_from_count`, `action_to_count`, `note`, `recorded_seconds`, `recorded_nanos`, `recorded_offset` | 감사 사건 추가 전용 원장. `Split`/`ReverseSplit` 주수 열은 `MONEY_SEGMENTS` 회피를 위해 `action_from_count`/`action_to_count`(앞자리 `0` 금지 양의 10진 `TEXT`, 최대 20자리)를 사용하며 `u64::MAX`까지 무손실 표현 |
| 사건 테이블 열·행 `CHECK` | 복합 제약 | 도메인 문법(`InstrumentId`·`KnownMarket`·`Symbol`·`IsinCode`·`DisclosureKey` DART/KIND 14자리 숫자·SEC EDGAR `10-2-6`·`UserReason` NUL 금지·UTF-8 1..512바이트·공백 전용 금지·`Timestamp` 초/나노초/분단위 오프셋) + `event_kind`별 필수/NULL 열 일치 + `link_corrected`·`identifier_changed` 대소문자 무시 변경 검증 + `SymbolChange`/`Split`/`ReverseSplit` 변형별 열 및 비율 방향 정합 | 깨진 사건 직렬화 행 삽입을 DB 계층에서 1차 차단(유니코드 공백·달력 일자 유효성·오프셋 적용 연도 범위·`u64::MAX` 초과는 Rust 디코더와 `domain`이 닫힘 검증) |
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
| `domain::verify_persisted_mapping_ledger(records, history)` | `InstrumentMappings`가 생성 가능한 모든 합법적 원장·이력 상태(비단조 감사 시각 포함) | `Ok(())` | 검증 전용(상태·토큰 미반환). 불일치·증거 위반·첨부 거래 포함·타 ID 겹침·고아 `Superseded`는 `Err(MappingLedgerIntegrityError)` |
| `domain::InstrumentMappings::link_confirmed_change` | `CorporateAction::with_receipt(...)` 첨부 거래가 있는 `Split`/`ReverseSplit`/`SymbolChange` | `Err(MappingTransitionError::InvalidEvidence)` | 영속 직렬화에서 첨부 거래가 조용히 유실되는 것을 막기 위해 도메인 검증 시점에 닫힘 거부 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| **읽기 전용 이력 포트 분리**: 기존 `InstrumentMappingRepository`는 변경하지 않고 `application::InstrumentMappingHistoryRepository`(`history`, `history_for_id`)를 별도 트레이트로 분리한다(2단계 쓰기 3연산 포트도 별도 분리 예정) | 일반 조회·동기화(`broker-toss`, `T-21`)와 감사 이력 조회, 그리고 향후 사용자 확인 쓰기 정정의 의존 경계를 분리한다. S-21·T-21의 기존 호출부·목·계약 테스트를 건드리지 않으면서 S-24 1단계 읽기 계약과 2단계 쓰기 계약을 단계적으로 쌓을 수 있다 | `InstrumentMappingRepository`에 `history`/`history_for_id`를 직접 추가하는 안(모든 기존 구현·가짜·T-21 테스트 세션에 이력 책임이 강제됨) | 메인 S-24 1단계 결정, `docs/adr/0157-instrument-identity-history.md` 정의 대기 9번 |
| **검증 전용 도메인 API(`verify_persisted_mapping_ledger`)로 D-55 봉인 보존**: 비봉인 `&[InstrumentMappingRecord]`·`&[MappingHistoryEvent]`를 받아 규칙대로 재생·대조한 뒤 `Result<(), MappingLedgerIntegrityError>`만 반환한다. `InstrumentMappings`나 `UserIdentityConfirmation`을 절대 돌려주지 않는다 | `InstrumentMappings` 복원 생성자를 열면 외부 크레이트가 `IssuedInstrumentId`·`UserIdentityConfirmation` 봉인을 거치지 않고 임의의 원장·이력을 조립해 도메인 메서드를 호출할 수 있게 되어 D-55 봉인이 깨진다(S-21 D-55 맞춤 리뷰 [1] 선례). 저장소는 `verify_persisted_mapping_ledger`로 원장↔사건 일관성만 검증하고, 실제 쓰기 상태 전이는 H-64 이후 `UserIdentityConfirmation`을 받은 2단계 쓰기 포트가 원자적 트랜잭션으로 수행한다 | `InstrumentMappings::restore_with_history(...)`로 도메인 원장 인스턴스를 반환하는 안(D-55 봉인 약화); `UserIdentityConfirmation` 공개 생성자를 여는 안(H-64 분리 결정 위반) | 메인 결정(A + C), `docs/adr/0157-instrument-identity-history.md`, `docs/adr/0158-instrument-mapping-store.md` |
| **`DisclosureKey::from_receipt(ReceiptNo)` 최소 생성자 추가**: `Disclosure` 본문·URL 없이 검증된 `ReceiptNo` 하나만으로 `DisclosureKey`를 복원한다 | ADR 0024의 핵심 불변식은 "`DisclosureKey`는 검증된 `ReceiptNo`를 봉인하며 열거형 판별자가 `source`를 결정하므로, 검증된 번호와 어긋난 출처를 전달할 수 없다"이다. `from_receipt(ReceiptNo)`는 `Dart(DartReceiptNo)`·`Kind(KindReceiptNo)`·`SecEdgar(SecEdgarReceiptNo)`의 사전 검증 타입만 받으므로 출처·번호 불일치를 원천 차단하며, 감사 사건 직렬화 복원 시 `CollectedItem`/`Url` 위조 없이 공시 키만 정확히 복원할 수 있다 | 저장소 복원용으로 가짜 URL·제목을 붙여 `Disclosure::try_new`를 호출하는 안(가짜 `CollectedItem` 생성으로 ADR 0024 취지 훼손); `DisclosureKey::from_raw(source, str)`를 여는 안(출처-번호 불일치 허용) | 메인 세부 결정, `docs/adr/0024-collected-information-types.md` |
| **`CorporateAction` 첨부 거래(`receipt()`) 거부**: `validate_change_action`에서 `action.receipt().is_some()`이면 `MappingTransitionError::InvalidEvidence`로 거부한다 | 식별자 변경 증거로 허용되는 `SymbolChange`·`Split`·`ReverseSplit`에는 원래 영수 거래(`Transaction`)가 없으나, `CorporateAction::with_receipt`로 첨부할 수 있었다. 사건 표에 첨부 거래를 저장하지 않으면 영속 왕복 시 조용히 유실되므로 도메인 검증 단계에서 닫힘 거부해 왕복 동등성을 보장한다 | 사건 표에 `Transaction` 직렬화 열을 추가하는 안(식별자 변경 증거에 불필요한 거래 스키마 중복); 영속 시 첨부 거래를 조용히 버리는 안(손실 직렬화) | 메인 세부 결정 |
| **`Split`/`ReverseSplit` 비율 열 이름으로 `action_from_count`/`action_to_count` 사용 및 정밀 양의 10진 `TEXT` 저장**: `ShareCount(u64)` 전 범위(`1..=u64::MAX`)를 앞자리 `0` 없는 10진 문자열로 저장한다 | SQLite `INTEGER`는 `i64::MAX`를 넘는 `u64` 값을 담지 못하며, 열 이름에 `ratio`·`shares` 대신 `count`를 사용해 `sql-check`의 금액 열(`MONEY_SEGMENTS`) 오탐과 무관한 제약을 피한다 | `INTEGER` 열 사용(`u64::MAX` 손실); `action_old_shares`/`action_ratio` 등 모호한 이름 사용 | 메인 세부 결정, `crates/xtask/src/sql_check/` |
| **감사 사건 시각(`recorded_seconds`, `recorded_nanos`, `recorded_offset`) 비단조 허용 및 무손실 보존**: 3개 `INTEGER` 열로 초·나노초·오프셋을 정확히 복원하되, 사건 간 시각 단조성은 검증하지 않는다 | ADR 0157 메인 결정 11번에 따라 NTP 시각 보정 등 벽시계 역행 상황에서도 정정이 막히지 않도록 단조 순번 `seq`를 유일한 주 순서로 삼는다 | `recorded_seconds >= prev.recorded_seconds` 트리거/검증 강제(ADR 0157 결정 11번 위배) | `docs/adr/0157-instrument-identity-history.md` 11번 |
| **`MigrationSchemaTracker`로 `0001..=0009` 누적 DDL 추적**: `ALTER TABLE ... RENAME TO`와 `DROP TABLE`을 반영해 현재 스키마 객체 집합을 계산하고 `sqlite_schema`와 대조한다 | `0009`가 `instrument_mappings`를 재생성하면서 `0008`의 이전 테이블·인덱스·트리거가 교체된다. 단순 누적 수집은 교체된 `0008` 객체를 중복 기대하게 되므로 `RENAME`/`DROP`을 반영해 최종 20개 매핑·이력·발급원 객체와 전체 16개 트리거를 정확히 검증한다. `MigrationSchemaTracker`는 현행 마이그레이션의 단일 줄 `CREATE TRIGGER`·명시 `ALTER TABLE ... RENAME TO`·`DROP TABLE` 형식을 전제하며, 어긋나면 `Database::open` 테스트에서 즉시 닫힘 실패한다 | `0009` 객체만 하드코딩 목록으로 대조하는 안(마이그레이션 원천과 이중 관리되어 드리프트 위험) | `docs/adr/0158-instrument-mapping-store.md` |

## 정의 대기 (선택지와 사용자 영향)

| 항목 | 선택지 | 사용자 영향 | 현재 처리 |
|---|---|---|---|
| 1. 2단계 쓰기 포트(`InstrumentMappingCorrectionRepository`)의 메서드 서명과 오류 열거형 (H-64 연동) | A) 도메인 요청 타입(`ConfirmedIdentifierChange`, `RetirementCorrection`, `LinkCorrection`)을 그대로 받는 별도 쓰기 포트 + `MappingTransitionStoreError { Transition(MappingTransitionError), Storage(SystemPortError) }` / B) `application` 전용 입력 DTO로 감싼 뒤 내부에서 도메인 규칙 검증 | 사용자 확인 화면·유스케이스가 오연결·폐지 정정 실패 이유를 얼마나 세밀하게 표시하고 재시도할지에 영향 | S-24 1단계에서는 읽기 전용 `InstrumentMappingHistoryRepository`만 정의하고, 쓰기 3연산 포트는 H-64 착륙 후 S-24 2단계에서 확정 |
| 2. 사건 이력 증가에 따른 `Database::open` 검증 비용 상한 | A) 현행처럼 열기 시 전체 `instrument_mapping_events` 재생·대조 유지(개인용 앱에서 식별자 변경·정정은 평생 수십~수백 건 수준) / B) 체크포인트 스냅샷 도입 | 앱 시작 시간(`PERF-1` 2초)에 영향 | 현행 O(N log N) 전체 재생·대조 유지; 수만 건 이상으로 커질 때 재검토 |
| 3. 로컬 오프라인 변조 위협 모델 한계(S-16 인계) | A) 열기 시점 스키마·사건 재생·`superseded_event_seq` 대조만 유지 / B) S-16 키체인 고정 HMAC 체인에 `instrument_mapping_events` 머리값을 결합 | 32바이트 DB 키와 오프라인 파일 쓰기 권한을 가진 공격자가 트리거 제거→사건+원장 동시 재작성→트리거 복구를 수행할 때 탐지 가능 여부에 영향 | 1단계는 로컬 스키마·재생 불변식 닫힘 검증을 제공하며, 오프라인 동시 재작성 탐지는 S-16 범위로 명시 |

## 2단계 인계 (H-64 착륙 후 S-24 2단계 범위)

1단계에는 `UserIdentityConfirmation` 발급 경계(H-64)가 없으므로 아래 쓰기 경로 및 시험 항목은 2단계로 인계한다. 1단계에서 확정한 `0009` 스키마·트리거·사건 직렬화 형식은 2단계가 스키마 변경 없이 그대로 사용한다.

1. **`application` 쓰기 포트 신설**: `InstrumentMappingCorrectionRepository`(가칭) — `link_confirmed_change`, `correct_retirement`, `correct_link` 3연산(H-64가 제공하는 `UserIdentityConfirmation` 소비).
2. **SQLite 쓰기 구현(`InstrumentMappingStore`)**: `BEGIN IMMEDIATE` 단일 트랜잭션에서 (a) `link_confirmed_change`: `INSERT event(identifier_changed)` → `UPDATE previous SET status='superseded', superseded_event_seq=seq` → `INSERT next(active)` + 도메인 전이 검증, (b) `correct_retirement`: `INSERT event(retirement_corrected)` → `UPDATE source SET status='active'` + `Retired` 검증, (c) `correct_link`: 최신 연결 LIFO 검증 + `INSERT event(link_corrected)` → `DELETE unlinked_next` → `UPDATE restored_previous SET status='active', superseded_event_seq=NULL`.
3. **쓰기 원자성·중간 실패·두 연결 경쟁 테스트**: `link_confirmed_change`/`correct_retirement`/`correct_link` 도중 트리거 중단·0행 영향 롤백(사건·원장 부분 반영 0건), 두 연결 동시 변경/정정/`ensure`/`retire` 경쟁, 잠금 시간 초과(`Unavailable`) 롤백.
4. **가짜·SQLite 쓰기 공유 계약(`testkit-ports`)**: `MemoryInstrumentMappingRepository`의 쓰기 3연산 구현 및 가짜·SQLite 동일 쓰기 계약 + 거짓 통과 방지 음성 테스트 + `MemoryInstrumentMappingHistoryStore`와 통합 검토.
5. **도메인↔SQLite 상태 기계 차등 속성 테스트**: H-64 보호 시험용 발급 가짜를 사용해 임의의 `ensure`/`retire`/`link_confirmed_change`/`correct_retirement`/`correct_link` 시퀀스 뒤 SQLite 재열기 및 `history`/`for_id`/`lookup`/`resolve` 완전 동등성 검증.

## 기존 테스트 변경 (기계적 고정 개수 갱신)

마이그레이션 `0009` 추가에 따라 마이그레이션 원장 개수와 `sqlite_master` 스키마 객체 개수를 고정 상수로 단언하는 기존 테스트 2건과 미래 마이그레이션 상수 2줄만 기계적으로 갱신했다(`crates/adapters/store-sqlite/src/instrument_mapping_tests.rs`는 0줄 변경, 기존 단언 삭제·약화 0건).

> 참고: 메인 지시대로 아래 2건의 승인 행은 구조 보존 규칙(`for`·`?`·클로저) 때문에 `approvals.toml`로 해소되지 않을 수 있으며, 메인이 병합 커밋에서 정확히 이 2건만 지적되는지 확인한다.

| 파일 | 변경 내용 | 기준 SHA-256 (`4c13d306`) | 변경 후 SHA-256 | `test-diff-fast` 지적 문자열 |
|---|---|---|---|---|
| `crates/adapters/store-sqlite/src/migration_tests.rs` | 마이그레이션 원장 개수 단언 `8` → `9` (1줄) | `d62952637a9c9e99a314ba9e8d229b89295351668e5c26d8004fd29f7160df8f` | `240cc894071421b0f04205cfba0dce04c67f4ff0880c5db44623c77330911a75` | `store-sqlite migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger: test-diff:modified:store-sqlite migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger` |
| `crates/adapters/store-sqlite/src/tests.rs` | `sqlite_master` 스키마 객체 개수 단언 `55` → `64` (+9 = 이벤트 테이블 1 + 이벤트 명시 인덱스 1 + 이벤트 트리거 3 + 원장 부분 고유 인덱스 4, 1줄) | `1f82f383ed3a04d89cb8059535fcf6676146399c89db050429dea9fdf1ba30e0` | `306405fff3ab503eeaaec63e626300deb913f81321040c382e19e33b3f02ef50` | `store-sqlite tests::raw_key_is_independently_readable_and_debug_is_closed: test-diff:modified:store-sqlite tests::raw_key_is_independently_readable_and_debug_is_closed` |
| `crates/adapters/store-sqlite/src/sql/holdings_tests.rs` | 동반 픽스처 상수 `GAP`/`FUTURE` `9` → `10` (2줄) | `aef1e733b1fa7b6852683064161ffb23488577cd9f3ff035748d81b0ac6a19ba` | `0dfb84a1310ca9a507be1868ec2c77a137b7fa2fb13313ce91341286e2560986` | 위반 없음 |
| `crates/adapters/store-sqlite/src/sql/instrument_mapping_tests.rs` | `0009` 무제약 테이블·트리거 복구 픽스처 SQL 2상수 동기화 (`instrument_mapping_tests.rs` 본체는 0줄 변경) | `9ace0df125e525f5db440271c52cb8e72fcd6d468a20bd48bda1926d7610f359` | `cf6600ff809fabe71baeabf106e1442cbf5ef5a035d9f7abb56a945b3626d158` | 위반 없음 |

제안 `harness/test-diff/approvals.toml` 행(구조 보존 규칙으로 해소되지 않을 수 있음):

```toml
[[approval]]
task = "S-24"
finding = "test-diff:modified:store-sqlite migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger"
path = "crates/adapters/store-sqlite/src/migration_tests.rs"
before_sha256 = "d62952637a9c9e99a314ba9e8d229b89295351668e5c26d8004fd29f7160df8f"
after_sha256 = "240cc894071421b0f04205cfba0dce04c67f4ff0880c5db44623c77330911a75"
assertion_change = true
reason = "마이그레이션 0009_instrument_mapping_history.sql 추가에 따라 원장 개수 기대값을 8에서 9로 갱신"

[[approval]]
task = "S-24"
finding = "test-diff:modified:store-sqlite tests::raw_key_is_independently_readable_and_debug_is_closed"
path = "crates/adapters/store-sqlite/src/tests.rs"
before_sha256 = "1f82f383ed3a04d89cb8059535fcf6676146399c89db050429dea9fdf1ba30e0"
after_sha256 = "306405fff3ab503eeaaec63e626300deb913f81321040c382e19e33b3f02ef50"
assertion_change = true
reason = "마이그레이션 0009 추가(+9: 이벤트 테이블 1, 이벤트 인덱스 1, 이벤트 트리거 3, 원장 부분 고유 인덱스 4)에 따라 스키마 객체 기대값을 55에서 64로 갱신"
```

## 실측 및 손 변형

(테스트·커버리지·손 변형·`mutants-daily` 완료 후 실측 값으로 채운다.)
