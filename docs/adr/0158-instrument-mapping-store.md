# ADR 0158: S-21 종목 내부 ID 대응과 발급원의 SQLite 저장소

- 상태: 확정 (2026-11-24)

- 날짜: 2026-11-24
- 결정한 사람: 구현 담당 초안(사용자·메인 확인 대기)
- 관련 작업: S-21 (연관 D-52, D-55, T-21, S-15, S-24)
- 반영한 문서: `crates/domain/src/instrument_issuer.rs`, `crates/adapters/store-sqlite/migrations/0008_instrument_mappings.sql`, `crates/adapters/store-sqlite/schema.snapshot.sql`, `crates/adapters/store-sqlite/migrations.lock`

## 맥락

D-52([docs/adr/0145-instrument-id-mapping.md](0145-instrument-id-mapping.md))와 D-55(`origin/merge/d-55` 커밋 `308f21d64d016cc956d6f71de6cad2fa7612635d`)는 종목의 시장·심볼·ISIN(`InstrumentSource`)을 앱 내부 불투명 식별자(`InstrumentId`)로 대응시키는 도메인 규칙(`InstrumentMappings`)과 독립 ID 발급원(`IndependentInstrumentIdIssuer`, 일회성 발급 토큰 `IssuedInstrumentId`), 비동기 포트(`InstrumentMappingRepository`, `InstrumentIdIssuer`)를 정의했다. 작업 S-21은 이 포트들을 `crates/adapters/store-sqlite`에 구현하면서 다음 제약을 모두 만족해야 한다.

- 발급(`ensure`)은 독립 발급원이 만든 일회성 토큰 `domain::IssuedInstrumentId`만 받고, `ensure`와 폐지(`retire`)는 한 SQLite 트랜잭션 안에서 원자적으로 처리되며 두 연결이 동시에 경쟁할 때도 단일 승자·폐지 불변성·재활성화 금지를 지킨다.
- 독립 발급원(`InstrumentIdIssuerStore`)은 SQLite 단조 순번 표(`instrument_id_issuer`)에 마지막 발급 순번을 영속화하고, 앱 재시작 및 두 연결 동시 발급에서도 같은 ID를 두 번 발급하지 않으며 활성·폐지된 기존 ID를 영구 예약해 재사용하지 않는다. 영속 커서와 분리된 가변 발급원을 외부에 내어 주는 공개 경로는 열지 않는다.
- DB 열기(`Database::open`)와 매 조회·갱신 시 정확 중복, 원본 겹침(같은 `(market, upper(symbol))` 또는 `upper(isin)`), 서로 다른 원본이 같은 `InstrumentId`를 공유하는 행, 도메인 문법 위반 값, 1:1 스키마에 도달 불가능한 고아 `superseded` 행, 손상된 발급원 커서를 거부하고 닫힘 실패(`SystemFailure::RepositoryIntegrity`)한다.
- 폐지된 대응의 이력은 삭제·덮어쓰기·재활성화 없이 보존하고, 활성 대응과 전체 이력의 고유 제약을 유지한다.
- `STRICT` 테이블, 왕복(재시작 포함), 깨진 값 거부 테스트를 갖추며, 가짜 구현(`MemoryInstrumentMappingRepository`, `MemoryGlobalInstrumentIds`)과 SQLite 구현(`InstrumentMappingStore`, `InstrumentIdIssuerStore`)이 동일한 포트 계약 테스트를 통과한다.
- DB에 저장된 값만으로 종목 마스터 조회를 거친 도메인 타입(`MasterInstrument`, 주문 해결 타입 등)을 만들지 않는다([docs/adr/0059-master-instrument.md](0059-master-instrument.md), [docs/adr/0104-holdings-snapshot-store.md](0104-holdings-snapshot-store.md)).
- 메인 결정으로 기존 `InstrumentMappingRepository`는 확장하지 않으며, D-55의 동일 ID 복수 원본(`Superseded` 승계 쌍)·이력(`history`)·연결(`link_confirmed_change`)·정정(`correct_retirement`, `correct_link`) 영속은 새 작업 **S-24**로 인계한다.

## 스키마와 제약

마이그레이션 `0008_instrument_mappings.sql`(SHA-256 `3ca8969d7037814b80623db44995726103048e074ca877cb92ba714d7cff0483`)이 생성하는 객체와 제약은 다음과 같다.

| 대상 | 종류 | 정의 | 역할 |
|---|---|---|---|
| `instrument_mappings` | `STRICT` 테이블 | `instrument_id TEXT PRIMARY KEY`, `market TEXT NOT NULL`, `symbol TEXT NOT NULL`, `isin TEXT NOT NULL`, `status TEXT NOT NULL`, `UNIQUE (market, symbol, isin)` | S-21 1:1 원장: 내부 ID 고유성, 원본 3-튜플 고유성, BLOB 등 타입 불일치 차단 |
| `instrument_id` `CHECK` | 열 제약 | `instr(instrument_id, char(0)) = 0 AND length(instrument_id) BETWEEN 1 AND 64 AND instrument_id NOT GLOB '*[^!-~]*'` | `domain::InstrumentId` 문법(ASCII 가시문자 1~64자, NUL 금지) 일치 |
| `market` `CHECK` | 열 제약 | `market IN ('KR', 'US')` | `domain::KnownMarket` 열거형 일치 |
| `symbol` `CHECK` | 열 제약 | `instr(symbol, char(0)) = 0 AND length(symbol) BETWEEN 1 AND 32 AND symbol GLOB '[A-Za-z0-9]*' AND symbol NOT GLOB '*[^A-Za-z0-9.-]*'` | `domain::Symbol` 문법(영숫자 시작, 영숫자·`.`·`-` 1~32자, NUL 금지) 일치 |
| `isin` `CHECK` | 열 제약 | `instr(isin, char(0)) = 0 AND length(isin) BETWEEN 1 AND 64 AND isin NOT GLOB '*[^!-~]*'` | `domain::IsinCode` 문법(ASCII 가시문자 1~64자, NUL 금지) 일치 |
| `status` `CHECK` | 열 제약 | `status IN ('active', 'retired')` | S-21 1:1 원장 허용 상태(승계 쌍·고아 `superseded` 미지원, S-24 인계) |
| `instrument_mappings_symbol_key` | `UNIQUE INDEX` | `(market, upper(symbol))` | 같은 시장 내 대소문자 변형 포함 심볼 중복·겹침 차단(활성·폐지 공통) |
| `instrument_mappings_isin_key` | `UNIQUE INDEX` | `(upper(isin))` | 전 시장 대소문자 변형 포함 ISIN 중복·겹침 차단(활성·폐지 공통) |
| `instrument_mappings_no_delete` | `BEFORE DELETE` 트리거 | 모든 삭제에 `RAISE(ABORT, ...)` | 활성·폐지 이력 삭제 차단 |
| `instrument_mappings_retire_only` | `BEFORE UPDATE` 트리거 | `OLD.status != 'active'`, `NEW.status != 'retired'`, 또는 식별 열 변경 시 `RAISE(ABORT, ...)` | `active -> retired` 단일 전이만 허용, 재활성화·재갱신·식별자 변경 차단 |
| `instrument_mappings_no_replace` | `BEFORE INSERT` 트리거 | 기존 `instrument_id`, `(market, upper(symbol))`, `upper(isin)` 충돌 시 `RAISE(ABORT, ...)` | `INSERT OR REPLACE`가 내부 삭제 후 재삽입으로 이력을 덮어쓰는 경로 차단 |
| `instrument_mappings_insert_active_only` | `BEFORE INSERT` 트리거 | `NEW.status != 'active'`이면 `RAISE(ABORT, ...)` | `retired` 상태로 직접 삽입해 수명 주기를 우회하는 경로 차단 |
| `instrument_id_issuer` | `STRICT` 단일행 테이블 | `slot INTEGER PRIMARY KEY CHECK (slot = 1)`, `last_issued TEXT NOT NULL CHECK (instr(last_issued, char(0)) = 0 AND length(last_issued) = 16 AND last_issued NOT GLOB '*[^0-9a-f]*' AND last_issued != '0000000000000000')` | `u64` 전 범위(`1..=u64::MAX`) 단조 발급 커서를 16자리 고정폭 소문자 16진수로 영속화(행 없음 = `Initial`) |
| `instrument_id_issuer_no_delete` | `BEFORE DELETE` 트리거 | 모든 삭제에 `RAISE(ABORT, ...)` | 발급 커서 삭제·초기화 차단 |
| `instrument_id_issuer_monotonic` | `BEFORE UPDATE` 트리거 | `NEW.slot != OLD.slot OR NEW.last_issued <= OLD.last_issued` 시 `RAISE(ABORT, ...)` | 커서 역행·제자리 갱신·슬롯 변조 차단(16자리 고정폭 16진수 사전순 = 부호 없는 `u64` 대소) |
| `instrument_id_issuer_single_insert` | `BEFORE INSERT` 트리거 | `EXISTS (SELECT 1 FROM instrument_id_issuer)` 시 `RAISE(ABORT, ...)` | `INSERT OR REPLACE` 등으로 기존 커서를 덮어쓰는 경로 차단 |

## 상황 → 결과

| 연산 | 상황 | 결과 | 비고 |
|---|---|---|---|
| `Database::open` | 정상 빈 DB 또는 정상 활성·폐지 행 및 정상 발급 커서 | `Ok(Database)` | 마이그레이션 `0008` 적용 및 `validate_on_open`(`lower(tbl_name) IN ('instrument_mappings', 'instrument_id_issuer')` DDL 11종 + DB 전체 비-table/index/view 객체 13종 대조 + O(n log n) `validate_ledger` + `validate_issuer_cursor`) 통과 |
| `Database::open` | 중복·겹침·공유 ID·문법 위반·고아 `superseded` 행, 손상된 발급 커서(0/대문자/길이 불일치/비-1 슬롯), 또는 테이블·인덱스·트리거 DDL 변조(대소문자 변형 `ON INSTRUMENT_MAPPINGS`, 타 테이블 침입 트리거, `writable_schema` 대소문자·BLOB·NUL 위장 `type` 포함) | `Err(SystemPortError { port: Repository, failure: RepositoryIntegrity })` | 열기 단계에서 스키마 객체·전체 트리거 집합·원장 불변식·발급 커서를 닫힘 검증 (DB 암호화 키 보유자의 트리거 삭제→재작성→재생성 한계는 아래 참조) |
| `InstrumentIdIssuerStore::issue()` | 초기 또는 기존 커서에서 다음 미사용 순번 존재 | `Ok(IssuedInstrumentId)` | `BEGIN IMMEDIATE` 트랜잭션에서 `IndependentInstrumentIdIssuer::restore(previous, &[])` 복원 후 `ID_EXISTS`(PK 인덱스 O(log n))로 활성·폐지·외부 편입 기존 ID를 건너뛰고 새 커서를 단조 저장 |
| `InstrumentIdIssuerStore::issue()` | 두 연결 동시 호출 또는 앱 재시작 뒤 연속 호출 | 전역 유일·단조 증가 `IssuedInstrumentId` | `BEGIN IMMEDIATE` 직렬화 + 영속 커서로 드롭·폐지된 ID 포함 절대 재발급 없음. 미폴링 드롭은 커서를 변경하지 않음 |
| `InstrumentIdIssuerStore::issue()` | 커서가 `ffffffffffffffff`(`u64::MAX`)에 도달해 소진 또는 쓰기 영향 행 수 `!= 1` | `Err(SystemPortError { port: Repository, failure: RepositoryIntegrity })` | 트랜잭션 롤백, 닫힘 실패(가짜 구현 `MemoryInstrumentIdIssuer`와 동일한 `RepositoryIntegrity` 닫힘 분류) |
| `resolve(source)` | 정확 일치 활성 행 존재 | `Ok(InstrumentId)` | |
| `resolve(source)` | 정확 일치 폐지 행 존재 | `Err(MappingStoreError::Mapping(Retired))` | |
| `resolve(source)` | 미등록 원본 | `Err(MappingStoreError::Mapping(Missing))` | |
| `resolve(source)` | 대소문자 변형 또는 심볼/ISIN 일부 겹침 | `Err(MappingStoreError::Mapping(IdentityUncertain))` | `domain::instrument_source_conflicts` 단일 원천으로 활성·폐지 행 겹침 검출 |
| `lookup(source)` | 정확 일치 활성·폐지 행 존재 | `Ok(InstrumentMappingRecord)` | 원본·ID·상태를 그대로 반환 |
| `lookup(source)` | 미등록 또는 겹침 | `Err(MappingStoreError::Mapping(Missing \| IdentityUncertain))` | |
| `for_id(id)` | 해당 ID 행 존재(활성·폐지 모두) | `Ok(Vec<InstrumentMappingRecord>)` | |
| `for_id(id)` | 해당 ID 없음 | `Err(MappingStoreError::Mapping(Missing))` | |
| `ensure(source, candidate: IssuedInstrumentId)` | 미등록 원본, 미사용 발급 후보 ID | `Ok(MappingAssignment::Issued(candidate_id))` | `BEGIN IMMEDIATE` 트랜잭션 내 `INSERT` 1행 확인 |
| `ensure(source, candidate)` | 이미 활성인 동일 원본(후보 ID가 달라도) | `Ok(MappingAssignment::Existing(existing_id))` | 기존 ID 유지, 새 행 미삽입 |
| `ensure(source, candidate)` | 이미 폐지된 동일 원본 | `Err(MappingStoreError::Mapping(Retired))` | 재활성화 금지 |
| `ensure(source, candidate)` | 심볼·ISIN·대소문자 겹침 | `Err(MappingStoreError::Mapping(IdentityUncertain))` | |
| `ensure(source, candidate)` | 다른 원본이 이미 사용 중인 후보 ID(활성·폐지 모두) | `Err(MappingStoreError::Mapping(IdAlreadyUsed))` | |
| `retire(source)` | 활성 동일 원본 | `Ok(id)` | `BEGIN IMMEDIATE` 트랜잭션 내 `UPDATE ... status = 'retired'` 1행 확인 |
| `retire(source)` | 이미 폐지된 동일 원본 | `Ok(id)` | 멱등, 추가 쓰기 없음 |
| `retire(source)` | 미등록 또는 겹침 | `Err(MappingStoreError::Mapping(Missing \| IdentityUncertain))` | |
| 두 연결 동시 `ensure` / `retire` / `issue` | 동일 원본·동일 후보 ID·겹침 원본·`ensure`/`retire` 경쟁·동시 `issue` | 한쪽만 승자, 반대쪽은 일관된 계약 결과; `issue`는 중복 0건 | `BEGIN IMMEDIATE`로 직렬화 |
| 모든 연산 | 잠금 대기 시간 초과(`SQLITE_BUSY` / `SQLITE_LOCKED`) | `Unavailable(RetryDelay::Unknown)` | 쓰기 롤백, 부분 상태 미노출 |
| `ensure` / `retire` / `issue` | 쓰기 트리거 실패 또는 영향 행 수 `!= 1` (`RAISE(IGNORE)` 포함) | `RepositoryIntegrity` | 트랜잭션 롤백, DB 상태 불변 |
| 모든 연산 | 조회·갱신 대상 키와 일치하는 행에 깨진 값·고아 `superseded`·중복·겹침·공유 ID 발생 | `RepositoryIntegrity` | 점조회 후보 행 검증에서 `validate_records`로 닫힘 실패 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| `STRICT` 테이블 `instrument_mappings`에 도메인 문법 `CHECK`, `status IN ('active', 'retired')`, 대소문자 무시 `UNIQUE INDEX`, 4개 보호 트리거를 둔다 | S-21의 1:1 `PRIMARY KEY(instrument_id)` 원장에서는 `Superseded` 승계 쌍(동일 `InstrumentId`에 `Superseded` + `Active` 2행 및 심볼/ISIN 겹침)을 저장할 수 없으므로, `0008`에 단독 `superseded` 행이 존재하면 곧 고아 변조 상태다. `active`/`retired`만 허용하고 고아 `superseded`는 닫힘 거부하며, `Superseded` 쌍·이력·정정 영속은 **S-24** 마이그레이션으로 일관되게 인계한다 | `0008` `CHECK`에 `'superseded'`를 허용하는 안(D-55 리뷰·비판에서 도메인이 생성 불가능한 고아 `Superseded` 행만 허용하고 실제 D-55 승계 쌍은 PK·UNIQUE INDEX가 거부하는 모순이 재현됨); 부분 인덱스(`WHERE status = 'active'`)만 두는 안(폐지된 원본의 심볼·ISIN 재사용 및 `IdAlreadyUsed`를 DB 계층에서 막지 못함) | `docs/adr/0145-instrument-id-mapping.md`, D-55, `docs/tasks.md` S-21, D-55 맞춤 리뷰 [2] |
| 독립 발급원 영속 형식으로 `STRICT` 단일행 테이블 `instrument_id_issuer`에 16자리 고정폭 소문자 16진수(`0000000000000001`..`ffffffffffffffff`) 단조 커서와 3개 보호 트리거를 두고, `InstrumentIdIssuerStore::issue`만 공개 발급 경로로 둔다(`restore_issuer()` 등 분리된 가변 발급원 반환 메서드는 두지 않는다) | SQLite `INTEGER`는 `i64::MAX`에서 포화해 `u64` 전 범위를 표현할 수 없다. 16자리 고정폭 소문자 16진수 `TEXT`는 `inst-{:016x}`와 1:1 일치하고 사전순 비교가 부호 없는 `u64` 순서와 같아 트리거 `NEW.last_issued <= OLD.last_issued`로 역행을 원천 차단한다. 매 발급은 `BEGIN IMMEDIATE` 안에서 `IndependentInstrumentIdIssuer::restore(previous, &[])`와 `ID_EXISTS`(PK 인덱스 O(log n))로 기존·폐지 ID를 건너뛰며 원자적으로 커서를 전진시킨다. 분리된 가변 `IndependentInstrumentIdIssuer`를 외부에 내어 주지 않아 저장소 발급원과 같은 순번을 이중 발급하는 경로를 원천 봉쇄한다 | `InstrumentIdIssuerStore::restore_issuer()`로 분리된 가변 발급원을 반환하는 안(D-55 맞춤 리뷰 [3]에서 `restored_domain.issue()`와 `reopened_issuer.issue()`가 동일 순번을 이중 발급함이 재현됨); SQLite `INTEGER`/`AUTOINCREMENT` 사용(`i64::MAX` 이후 불일치); 매 `issue()`마다 전체 테이블 로드(O(n)/호출) | 메인 D-55 맞춤 결정, D-55 맞춤 리뷰 [3] |
| `crates/domain`에는 발급원 영속 복원용 최소 API(`IssuerSequence`, `IssuerSequenceError`, `PersistedIssuerCursor`, `IndependentInstrumentIdIssuer::restore`, `IndependentInstrumentIdIssuer::last_issued`)만 추가하고, `InstrumentMappings`는 일절 변경하지 않는다(`InstrumentMappings::restore` 미추가) | `InstrumentMappings`에 `restore(Vec<InstrumentMappingRecord>)`를 열면 어떤 크레이트든 `IssuedInstrumentId`·`UserIdentityConfirmation` 봉인을 거치지 않고 임의 심볼 파생 ID나 비정합 `Superseded` 장부를 만들 수 있다(D-55 맞춤 리뷰 [1]·[2] 재현). `InstrumentMappings`를 `308f21d6` 그대로 보존하고 `store-sqlite`가 `domain::instrument_source_conflicts` 단일 원천으로 충돌 판정·원장 검증을 수행하면 D-55의 `InstrumentMappings` 봉인이 1비트도 약화되지 않는다. 발급원 복원의 허용 범위: `IssuerSequence::try_new` + `restore(LastIssued(n))`는 영속 어댑터의 순번 복원을 위해 `inst-{:016x}` 형식 내부의 순번 복원을 허용하되, 형식 밖 임의·심볼 파생 ID 위조는 여전히 컴파일 차원에서 차단되며 전역 유일성은 SQLite 직렬화 경계가 담당한다. 또한 `IndependentInstrumentIdIssuer::issue()`는 `Exhausted` 실패 시 `self.next`를 변이시키지 않는다 | `domain::InstrumentMappings::restore` 공개 추가(D-55 봉인 우회 및 승계 쌍 불일치 재현); `IssuedInstrumentId::from_raw` 공개 추가(임의 ID 위조 허용) | 메인 D-55 맞춤 결정, D-55 맞춤 리뷰 [1]·[2]·[4]·추가 2, `AGENTS.md` 7번 |
| `Database::open`(`migration::apply_guarded`)에서 `instrument_mappings`·`instrument_id_issuer` DDL 11종(`lower(tbl_name) IN (...)`), DB 전체 비-table/index/view 스키마 행(`typeof(type) != 'text' OR type NOT IN ('table', 'index', 'view')`, `MIGRATIONS` 선언 트리거 13종), O(n log n) `validate_ledger`(`validate_records` + `domain::instrument_source_conflicts`), `validate_issuer_cursor`를 1회 검증하고, 개별 포트 연산은 SQLite 인덱스 기반 점조회(`BY_SOURCE`, `BY_SOURCE_OR_ID`, `BY_ID`, `ID_EXISTS`)만 수행한다 | 열기 시점에 스키마 변조(대소문자 변형 `ON INSTRUMENT_MAPPINGS`, 타 테이블 부착 침입 트리거, `PRAGMA writable_schema=ON`으로 주입한 `'Trigger'`·BLOB·NUL 접미어 위장 `type` 포함)와 원장·커서 손상을 O(n log n)으로 닫힘 거부한다. **위협 모델 한계**: 32바이트 SQLCipher 원시 키와 오프라인 파일 쓰기 권한을 가진 공격자가 `DROP TRIGGER` → 행 재작성 → 동일 문장으로 트리거 재생성을 수행하면 열기 시점 스키마·불변식 검증만으로는 과거 이력 변조를 탐지할 수 없으며, 이는 키체인 고정 MAC(S-16)의 범위다. 또한 `expected_migration_triggers`는 현행 `MIGRATIONS`의 단일 줄 `CREATE TRIGGER` 형식을 가정한다(어긋나면 테스트에서 즉시 닫힘 실패). T-21 `/stocks/all`의 수천~1만 종목 연속 조회·발급에서도 SQLite `MULTI-INDEX OR` 인덱스를 타 O(log n)/호출로 `PERF-1`·`PERF-4`를 지킨다 | 매 호출마다 전체 원장을 O(n²)로 재복원하는 안(1차 실측 n=5,000 기준 평균 264.5ms/호출, `PERF-4` 위반); `tbl_name = ...` 대소문자 구분 필터만 쓰는 안(2차 실측 우회); `WHERE type = 'trigger'` 또는 `WHERE lower(type) = 'trigger'`만 쓰는 안(3차 실측 `writable_schema` 위장 통과) | `docs/tasks.md` S-21, `docs/design.md` PERF-1·PERF-4, `AGENTS.md` 7번 |
| 기존 `InstrumentMappingRepository` 포트는 확장하지 않고 현행 포트 연산(`resolve`, `lookup`, `for_id`, `ensure`, `retire`)과 `InstrumentIdIssuer`만 구현하며, D-55 변경 이력·정정 연산(`history`, `link_confirmed_change`, `correct_retirement`, `correct_link`)과 `Superseded` 승계 쌍 영속은 **S-24**로 인계한다 | 메인 결정으로 D-55 정의 대기 9번(영속 포트 확장 형태)을 S-21에서 임의 확정하지 않고 분리한다 | S-21에서 임의로 `InstrumentMappingRepository`에 이력·정정 메서드를 추가하는 안 | 메인 D-55 맞춤 결정 |
| `ensure`, `retire`, `issue`는 `Database::write`(`BEGIN IMMEDIATE`) 단일 트랜잭션에서 수행한다 | WAL 모드에서 두 연결 동시 읽기→쓰기 승격 교착(`SQLITE_BUSY_SNAPSHOT`)과 TOCTOU 중복 발급·재활성화를 막는다 | `DEFERRED` 트랜잭션 사용; 읽기·쓰기 트랜잭션 분리 | `docs/tasks.md` S-21, `crates/adapters/store-sqlite/src/worker.rs` |
| 저장소는 `InstrumentSource`, `InstrumentId`, `IssuedInstrumentId`, `InstrumentMappingRecord`만 입출력하고 `MasterInstrument`나 주문 해결 타입을 만들지 않는다 | ADR 0059·0104의 마스터 검증 경계를 보존한다 | 대응 저장소에서 `MasterInstrument`를 조립하는 안 | `docs/adr/0059-master-instrument.md`, `docs/adr/0104-holdings-snapshot-store.md`, `docs/adr/0150-instrument-master.md` |
| `crates/testkit-ports/src/contracts/instrument_mapping.rs`에 대응·발급원 공유 계약 테스트를 두고 가짜 구현과 SQLite 구현 양쪽에서 실행한다 | D-52·D-55 가짜 구현과 SQLite 구현의 계약 일치 및 두 연결 경쟁·중단(미폴링 드롭)·재시작 유일성을 동일 기준으로 보장한다. 사전 등록 폐지·활성 ID 건너뛰기 검증은 미폴링 드롭과 분리해 거짓 통과를 차단하고(`issuer_contract_rejects_eager_non_skipping_issuer` 회귀 테스트로 고정) | 크레이트별 중복 테스트(계약 드리프트 위험) | `docs/tasks.md` S-21, D-55 맞춤 비판 추가 1 |

## 정의 대기 (선택지와 사용자 영향)

문서(`docs/open-issues.md`, `docs/tasks.md`, `docs/adr/0145-instrument-id-mapping.md`, `docs/adr/0150-instrument-master.md`)에 아직 확정되지 않았거나 후속 작업으로 분리된 항목은 아래와 같다.

| 항목 | 선택지 | 사용자 영향 | 현재 처리 |
|---|---|---|---|
| 1. D-55 변경 이력·정정 연산(`history`, `link_confirmed_change`, `correct_retirement`, `correct_link`) 및 동일 `InstrumentId` 복수 원본(`Superseded` 승계 쌍) 영속화 (**S-24 인계**) | A: 별도 영속 포트 또는 확장 트레이트 + `0009` 마이그레이션으로 `instrument_mappings` PK 확장(`(instrument_id, market, symbol, isin)`), 부분/승계 허용 인덱스 및 이력 이벤트 테이블(`instrument_mapping_history`) 추가 / B: `instrument_mappings`는 현행 유지하고 별도 `instrument_aliases`·이력 테이블로 승계·정정 관리 | 티커 변경·액면병합·ISIN 변경·폐지/연결 정정 시 기존 메모·보유 내역·거래 기록이 같은 내부 ID로 이어지고 감사 이력이 영속되는 방식에 직접 영향 | 메인 결정으로 S-21은 1:1 `active`/`retired` 원장과 `InstrumentIdIssuer`만 구현하고 고아 `superseded`는 무결성 거부. 승계 쌍·이력·정정 영속과 스키마·도메인 복원 확장은 새 작업 **S-24**로 인계 |
| 2. 기존 테이블(`market_quotes`, `market_candles`, `information_links`, `thesis_notes`)의 기존 `instrument_id` 편입 및 테이블 간 FK 정책 | A: 별도 마이그레이션에서 기존 행을 백필한 뒤 FK 추가 / B: `instrument_mappings`를 마스터 경계 원장으로 유지하고 기존 캐시·메모 테이블은 점진 전환 / C: 기존 캐시 테이블 초기화 후 신규 ID만 저장 | 기존에 저장된 시세 캐시·메모·정보 링크가 마이그레이션 후 유지되는지, 미등록 ID 기록이 차단되는지에 직접 영향 | 마이그레이션 `0008`은 기존 테이블 스키마·데이터를 변경하지 않고 독립 테이블 `instrument_mappings`·`instrument_id_issuer`만 추가 |
| 3. 종목 마스터 권한 세션(`InstrumentMasterAuthority` / `InstrumentMasterSession`)의 토스 실측 근거와 네트워크 대기 중 잠금 범위 (ADR 0150 인계) | A: 마스터 조회는 DB 잠금 밖에서 수행하고 결과 확정 시점에만 짧은 `BEGIN IMMEDIATE` 트랜잭션 사용 + 호출자 측 단일 비행(single-flight) / B: 직렬화된 권한 세션 큐에서 조회와 대응 발급을 순차 실행 | 종목 검색·동기화 응답 지연과 동시 조회 시 대기 시간, 그리고 네트워크 오류 시 DB 쓰기 차단 여부에 영향 | `InstrumentMappingStore`·`InstrumentIdIssuerStore`는 순수 DB 트랜잭션만 짧게 수행하며, 마스터 조회·세션 직렬화 계층은 토스 실측 및 후속 작업으로 유지 |
| 4. 원장·발급원 무결성 오류(`RepositoryIntegrity`)와 T-21 최종 마스터 응답 검증 오류의 상위 구분·복구 절차 (ADR 0150 인계) | A: 앱 실행 차단 후 백업 복원·DB 재구축 안내 / B: 대응 원장 읽기 전용 진단 모드 제공 / C: 마스터 응답 불일치만 해당 종목 거래 차단으로 격리하고 DB 무결성 오류는 전체 중단 | DB 손상 또는 브로커 응답 이상 발생 시 앱이 어디까지 안전하게 멈추고 사용자가 어떤 복구 단계를 밟아야 하는지에 영향 | `store-sqlite`는 원장·커서 손상 및 순번 소진을 일관되게 `SystemFailure::RepositoryIntegrity`로 보고하며 UI/상위 복구 흐름은 후속 작업으로 남김 |
| 5. 다중 DB 연결 경쟁 시 명시적 `busy_timeout` 값과 재시도 정책 | A: rusqlite 기본값(5초) 유지 / B: 연결 열기 시 명시적 `PRAGMA busy_timeout` 고정(예: 5000ms) + 상위 재시도 정책 분리 / C: 짧은 타임아웃 + 지수 백오프 재시도 | 다중 창·백그라운드 동기화와 사용자 조작이 겹칠 때 일시 대기 시간과 "일시 사용 불가" 오류 빈도에 영향 | `connection.rs` 기존 설정(rusqlite 기본 5초 `busy_timeout`)을 변경하지 않고, 초과 시 `SystemFailure::Unavailable(RetryDelay::Unknown)` 반환 |

## 기존 테스트 변경 (기계적 고정 개수 갱신)

마이그레이션 `0008` 추가(`instrument_mappings` + `instrument_id_issuer`)에 따라 전체 마이그레이션 수 및 `sqlite_master` 스키마 객체 수를 고정 상수로 검증하는 기존 테스트만 기계적으로 갱신했다(단언 삭제·약화 없음; 기준 `308f21d64d016cc956d6f71de6cad2fa7612635d` 대비 `domain`·`application`·`testkit-ports` 위반 0건).

| 파일 | 테스트/상수 | 전 → 후 | `xtask test-diff` 지적 문자열 (기준 `308f21d6`) |
|---|---|---|---|
| `crates/adapters/store-sqlite/src/migration_tests.rs` | `fresh_open_records_clock_and_reopen_preserves_ledger` (`LEDGER_COUNT`) | `7` → `8` | `store-sqlite migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger: test-diff:modified:store-sqlite migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger` |
| `crates/adapters/store-sqlite/src/tests.rs` | `raw_key_is_independently_readable_and_debug_is_closed` (스키마 객체 수) | `42` → `55` (테이블 2 + 명시 인덱스 2 + 자동 인덱스 2 + 트리거 7 = +13) | `store-sqlite tests::raw_key_is_independently_readable_and_debug_is_closed: test-diff:modified:store-sqlite tests::raw_key_is_independently_readable_and_debug_is_closed` |
| `crates/adapters/store-sqlite/src/sql/holdings_tests.rs` | `GAP` / `FUTURE` (미래 마이그레이션 번호 상수) | `0008_...` → `0009_...` (`8` → `9`) | (`test-diff` 미지적; `migration_tests`의 격차·미래 버전 검사 입력 상수) |

## 실측

- 도구: `cargo 1.99.1`, `cargo-nextest 0.9.151`, `node v24.22.0`, `just 1.59.0`
- 커버리지 (`cargo llvm-cov nextest --locked -p domain -p store-sqlite --summary-only`):
  - 1차: `store-sqlite/src/instrument_mapping.rs` 줄 172/172(100.00%), 함수 25/25(100.00%), 영역 263/271(97.05%)
  - 2차: `store-sqlite/src/instrument_mapping.rs` 줄 267/269(99.26%), 함수 36/36(100.00%)
  - D-55 맞춤 최종: `domain/src/instrument_issuer.rs` 영역 138/138(100.00%), 함수 15/15(100.00%), 줄 95/95(100.00%); `domain/src/instrument_mapping.rs` 영역 424/424(100.00%), 함수 41/41(100.00%), 줄 341/341(100.00%, `308f21d6` 대비 변경 0줄); `store-sqlite/src/instrument_mapping.rs` 함수 64/64(100.00%), 줄 429/433(99.08%), 영역 657/687(95.63%)
- 자동 변경분 변이 (`just mutants-daily 308f21d64d016cc956d6f71de6cad2fa7612635d 0/1`):
  - 선택 파일: `crates/domain/src/instrument_issuer.rs`, `crates/domain/src/lib.rs` (선택 변이 79개)
  - 결과: `79 mutants tested in 17m: 51 caught, 28 unviable` / `mutants: 위반 없음` (생존 0건, 시간 초과 0건)
- Release `n=5,000` 성능 실측 (격리 사본, SQLCipher 암호화 DB, `cargo test --locked --release`):
  - 1차(매 호출 전체 원장 `InstrumentMappings::ensure` O(n²)): 열기 `190.1ms`, `resolve` 평균 `264.5ms`(5,000회 추정 `1,322.7s`), `ensure` 신규 `339.4ms`
  - 2차 구현자 실측(열기 `validate_schema` + O(n log n) `validate_ledger`, 포트 연산 인덱스 점조회): 열기 `28.39ms`(6.7배 단축), `resolve` 5,000회 `415.76ms`(평균 `0.0832ms`, 3,179배 단축), `ensure` 기존 500회 `33.97ms`, `ensure` 신규 `0.36ms`(943배 단축)
  - 2차 독립 비판자 재현 실측(동일 조건): 열기 `24.71ms`, `resolve` 5,000회 `436.46ms`(평균 `0.0873ms`), `ensure` 기존 500회 `48.62ms`, `ensure` 신규 10회 평균 `0.2648ms`; `EXPLAIN QUERY PLAN` 및 VDBE 바이트코드에서 `BY_SOURCE`·`BY_SOURCE_OR_ID`·`BY_ID`·`ID_EXISTS`와 `instrument_mappings_no_replace` 트리거 하위 질의 모두 인덱스 사용(`SCAN instrument_mappings` 0건)을 확인하고 테스트(`indexed_point_queries_use_indexes_without_table_scan`)로 고정했다.
- 손 변형 (격리 사본 `target/S-21/work`에서 실제 실행 후 사본 삭제): 1차 26건(M01~M26) + 2차 12건(M27~M38) + 3차 5건(M39~M43) + 3차 수정 2건(M44~M45) + D-55 맞춤 15건(M46~M60, 리뷰 수정 뒤 해당 구간 재검증 포함) = 총 60건 검출(생존 0건). 상세 표는 아래 참조.
  - 1차 초기 실행에서 M20·M21(`symbol_key`·`isin_key` 고유 인덱스 제거)이 `no_replace` 트리거에 가려 생존했으나, 보호 트리거 제거 후 인덱스 단독 검증 테스트(`sqlite_unique_indexes_enforce_case_insensitive_keys_without_triggers`)를 추가해 26/26 검출을 확인했다.
  - 2차 초기 실행에서 M27(`validate_schema`의 본문 비교 생략)과 M33(`validate_on_open`의 `validate_ledger` 생략이 `fts_schema::validate`의 `PRAGMA quick_check`에 가려짐)이 생존했으나, 같은 객체 수로 트리거 본문만 약화한 `REPLACE_WEAK_NO_DELETE_TRIGGER` 케이스와 `validate_on_open` 직접 단언을 추가해 12/12 검출을 확인했다.
  - 3차에서는 `lower(tbl_name)` 대소문자 무시와 DB 전체 트리거 대조(`validate_mapping_objects`·`validate_all_triggers`)에 대한 5건(M39~M43)을, 3차 수정에서는 `writable_schema` 대소문자·BLOB·NUL 접미어 위장 `type` 차단(`typeof(type) != 'text' OR type NOT IN ('table', 'index', 'view')`)에 대한 2건(M44~M45)을, D-55 맞춤에서는 `domain::instrument_issuer` 복원 API·`InstrumentIdIssuerStore`·고아 `superseded` 거부·`instrument_id_issuer` 트리거에 대한 손 변형과 자동 변경분 변이(`just mutants-daily 308f21d64d016cc956d6f71de6cad2fa7612635d 0/1`)를 완주했다.
  - 참고: 정상 스키마에서는 `PRIMARY KEY`·`UNIQUE`·`UNIQUE INDEX`가 일반 SQL 중복·겹침 삽입을 이미 차단하므로, 끝단 `Database::open` 경로의 `validate_on_open -> validate_ledger` 연결은 정상 스키마 + 손상 열 테스트(`open_and_operations_reject_corrupted_column_values`, `open_and_operations_reject_orphan_superseded_status_in_s21_schema`)로 검증하고, 중복·겹침·공유 ID 13종은 무제약 테이블에서 `validate_ledger` 및 영향 키 점조회 경로(`validate_ledger_accepts_valid_unindexed_rows_and_rejects_duplicate_overlap_or_shared_id`)로 검증한다.

| ID | 대상·변형 내용 | 결과 | 검출 테스트(대표) |
|---|---|---|---|
| M01 | `market_tag`: `Kr => "US"` 오매핑 | 검출 | `sqlite_store_passes_shared_collision_contract` 외 |
| M02 | `decode_market`: `"KR" => Us` 오매핑 | 검출 | `sqlite_store_passes_shared_concurrency_contracts_on_single_connection` 외 |
| M03 | `decode_market`: 미지 시장값 `Ok(Kr)` 허용 | 검출 | `open_and_operations_reject_corrupted_column_values` |
| M04 | `decode_status`: `active`/`retired` 뒤바꿈 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M05 | `decode_status`: 미지 상태값 `Ok(Active)` 허용 | 검출 | `open_and_operations_reject_corrupted_column_values`, `open_and_operations_reject_orphan_superseded_status_in_s21_schema` |
| M06 | `validate_records`: 정확 중복 묵인 | 검출 | `validate_ledger_accepts_valid_unindexed_rows_and_rejects_duplicate_overlap_or_shared_id` |
| M07 | `validate_records`: 원본 겹침·공유 ID 오류 묵인 | 검출 | `validate_ledger_accepts_valid_unindexed_rows_and_rejects_duplicate_overlap_or_shared_id` |
| M08 | `StoredStatus::into_domain`: `retired` 행의 폐지 상태 미복원 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M09 | `ensure_mapping`: 신규 발급 시 `INSERT` 생략 | 검출 | `sqlite_store_passes_shared_collision_contract` 외 |
| M10 | `ensure_mapping`: `inserted != 1` 검증 제거 | 검출 | `injected_write_failures_and_zero_row_effects_roll_back_cleanly` |
| M11 | `ensure_mapping`: 신규 행을 `retired`로 삽입 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M12 | `retire_mapping`: 이미 폐지된 원본 멱등성 제거 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M13 | `retire_mapping`: 활성 행 `UPDATE` 생략 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M14 | `retire_mapping`: `updated != 1` 검증 제거 | 검출 | `injected_write_failures_and_zero_row_effects_roll_back_cleanly` |
| M15 | `map_result`: 도메인 오류를 `Missing`으로 뭉갬 | 검출 | `sqlite_store_passes_shared_collision_contract` 외 |
| M16 | `resolve_in_candidates`: 폐지 행도 ID 반환 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M17 | `for_id`: 미존재 시 빈 벡터 반환 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M18 | `migration::apply_guarded`: 열기 시점 원장 검증 제거 (2차 M37로 계승) | 검출 | `validate_ledger_accepts_valid_unindexed_rows_and_rejects_duplicate_overlap_or_shared_id` |
| M19 | DDL: `STRICT` 테이블 옵션 제거 | 검출 | `strict_table_and_valid_domain_boundaries_roundtrip` 및 스키마 검증 |
| M20 | DDL: `(market, upper(symbol))` 고유 인덱스 제거 | 검출 | `sqlite_unique_indexes_enforce_case_insensitive_keys_without_triggers` |
| M21 | DDL: `(upper(isin))` 고유 인덱스 제거 | 검출 | `sqlite_unique_indexes_enforce_case_insensitive_keys_without_triggers` |
| M22 | DDL: `no_delete` 트리거 제거 | 검출 | `sqlite_triggers_protect_identity_and_retired_history` |
| M23 | DDL: `retire_only` 트리거 제거 | 검출 | `sqlite_triggers_protect_identity_and_retired_history` 외 |
| M24 | DDL: `no_replace` 트리거 제거 | 검출 | `sqlite_no_replace_trigger_blocks_conflict_replacement` 외 |
| M25 | DDL: `insert_active_only` 트리거 제거 | 검출 | `sqlite_constraints_protect_uniqueness_and_block_direct_retired_insert` 외 |
| M26 | DDL: `symbol` NUL `CHECK` 제거 | 검출 | `check_constraints_reject_invalid_fields_and_nul` |
| M27 | `validate_schema`: `actual != expected` 내용 비교 생략 | 검출 | `open_rejects_tampered_mapping_schema_objects` |
| M28 | `validate_schema`: 스키마 검증 무효화(항상 `Ok`) | 검출 | `open_rejects_tampered_mapping_schema_objects` |
| M29 | `validate_records`: `seen_ids` 중복 검사 생략 | 검출 | `validate_ledger_accepts_valid_unindexed_rows_and_rejects_duplicate_overlap_or_shared_id` |
| M30 | `validate_records`: `seen_sources` 정확 중복 검사 생략 | 검출 | `validate_ledger_accepts_valid_unindexed_rows_and_rejects_duplicate_overlap_or_shared_id` |
| M31 | `validate_records`: `instrument_source_conflicts` 겹침 거부 생략 | 검출 | `validate_ledger_accepts_valid_unindexed_rows_and_rejects_duplicate_overlap_or_shared_id` |
| M32 | `validate_on_open`: `validate_schema` 호출 생략 | 검출 | `open_rejects_tampered_mapping_schema_objects` |
| M33 | `validate_on_open`: `validate_ledger` 호출 생략 | 검출 | `open_and_operations_reject_corrupted_column_values` |
| M34 | `sql::BY_SOURCE`: `upper(isin)` 조건 누락 | 검출 | `sqlite_store_passes_shared_overlap_and_case_contract` 외 |
| M35 | `sql::BY_SOURCE_OR_ID`: `instrument_id` 조건 누락 | 검출 | `sqlite_store_passes_shared_collision_contract` 외 |
| M36 | `sql::BY_SOURCE`: `upper(symbol)` 대소문자 정규화 제거 | 검출 | `sqlite_store_passes_shared_overlap_and_case_contract` 외 |
| M37 | `migration::apply_guarded`: `validate_on_open` 호출 제거 | 검출 | `open_rejects_tampered_mapping_schema_objects` 외 |
| M38 | `testkit-ports`: `concurrent_overlapping_sources` 승자 0건 fallthrough 재주입 | 검출 | `overlapping_race_contract_rejects_double_uncertain_without_winner` 외 |
| M39 | `sql::SCHEMA_OBJECTS`: `lower(tbl_name)` 대소문자 무시 제거 | 검출 | `open_rejects_tampered_mapping_schema_objects` |
| M40 | `validate_schema`: `validate_all_triggers` 호출 생략 | 검출 | `open_rejects_tampered_mapping_schema_objects` |
| M41 | `validate_schema`: `validate_mapping_objects` 호출 생략 | 검출 | `open_rejects_tampered_mapping_schema_objects` |
| M42 | `validate_sql_set`: `actual != *expected` 내용 비교 생략 | 검출 | `open_rejects_tampered_mapping_schema_objects` |
| M43 | `validate_all_triggers`: 전체 트리거 검증 무효화(항상 `Ok`) | 검출 | `open_rejects_tampered_mapping_schema_objects` |
| M44 | `sql::ALL_TRIGGERS`: `WHERE type = 'trigger'` 복귀(대소문자·BLOB·NUL 위장 허용) | 검출 | `open_rejects_writable_schema_disguised_triggers` |
| M45 | `sql::ALL_TRIGGERS`: `WHERE lower(type) = 'trigger'` 약화(NUL 접미어 위장 허용) | 검출 | `open_rejects_writable_schema_disguised_triggers` |
| M46 | `domain::IssuerSequence::try_new`: `0` 허용 | 검출 | `domain::instrument_persistence_restore` |
| M47 | `domain::IndependentInstrumentIdIssuer::restore`: `LastIssued` 커서 무시 | 검출 | `domain::instrument_persistence_restore` |
| M48 | `domain::IndependentInstrumentIdIssuer::restore`: `existing` ID 편입 생략 | 검출 | `domain::instrument_persistence_restore` |
| M49 | `domain::IndependentInstrumentIdIssuer::last_issued`: `u64::MAX` 소진 시 `Initial` 오반환 | 검출 | `domain::instrument_persistence_restore` |
| M50 | `domain::IndependentInstrumentIdIssuer::issue`: `Exhausted` 시 `self.next` 선행 변이 | 검출 | `domain::instrument_persistence_restore` (`issuer_handles_u64_max_boundary_without_mutating_cursor_on_exhaustion`) |
| M51 | `store-sqlite`: `lookup_in_candidates` 겹침 판정 생략 | 검출 | `sqlite_store_passes_shared_overlap_and_case_contract` 외 |
| M52 | `store-sqlite`: `ensure_mapping` 예약 ID 충돌(`IdAlreadyUsed`) 검사 생략 | 검출 | `sqlite_store_passes_shared_collision_contract` 외 |
| M53 | `store-sqlite`: `encode_sequence_hex` 하위 니블 마스킹 왜곡(`& 7`) | 검출 | `issuer_skips_preexisting_external_and_retired_ids_and_exhausts_at_u64_max` |
| M54 | `store-sqlite`: `decode_sequence_hex` 길이/문자 조건 `\|\|` → `&&` 약화 | 검출 | `open_rejects_corrupted_issuer_cursor_with_intact_schema` |
| M55 | `store-sqlite`: `issue_next` 기존 ID 존재 건너뛰기 생략 | 검출 | `issuer_skips_preexisting_external_and_retired_ids_and_exhausts_at_u64_max` |
| M56 | `store-sqlite`: `validate_on_open`에서 `validate_issuer_cursor` 결과 무시 | 검출 | `open_rejects_corrupted_issuer_cursor_with_intact_schema` |
| M57 | `store-sqlite`: `decode_status`에서 고아 `superseded` 허용 | 검출 | `open_and_operations_reject_orphan_superseded_status_in_s21_schema` |
| M58 | `store-sqlite`: `persist_issuer_cursor`에서 `affected != 1` 검증 생략 | 검출 | `injected_issuer_zero_row_effects_fail_closed` |
| M59 | `testkit-ports`: `issuer_persists_uniqueness_and_skips_retired_and_existing`의 선행 미폴링 드롭 복귀(거짓 통과) | 검출 | `issuer_contract_rejects_eager_non_skipping_issuer` |
| M60 | DDL: `instrument_id_issuer_monotonic` 트리거 제거 | 검출 | `issuer_table_constraints_and_triggers_block_tampering` |

## 결과

- `crates/domain`에 `InstrumentMappings` 봉인을 일절 건드리지 않는 최소 발급원 복원 API(`IssuerSequence`, `IssuerSequenceError`, `PersistedIssuerCursor`, `IndependentInstrumentIdIssuer::restore`/`last_issued`)가 추가된다.
- `crates/adapters/store-sqlite`에 `InstrumentMappingStore`, `InstrumentIdIssuerStore`, 마이그레이션 `0008_instrument_mappings.sql`(`instrument_mappings` + `instrument_id_issuer`)이 추가된다.
- `crates/testkit-ports/src/contracts/instrument_mapping.rs`의 대응·발급원 공유 계약 테스트가 가짜 구현과 SQLite 구현 양쪽의 회귀 기준선이 된다.
- 후속 작업: **S-24**(D-55 동일 `InstrumentId` 복수 원본·`Superseded` 승계 쌍·`history`·`link_confirmed_change`·`correct_retirement`·`correct_link` 영속 및 스키마 확장), S-15(종목 마스터·대응 연결), T-21 후속(토스 실측 및 권한 세션 직렬화).
- 바뀌면 다시 볼 조건: S-24가 단일 `InstrumentId`에 복수 원본(변경 이력)을 허용하거나, 기존 테이블에 `instrument_mappings(instrument_id)` 외래 키를 도입하거나, 열기 시점 전체 원장 O(n log n) 검증 비용이 `PERF-1`(2초)에 근접할 만큼 수십만 행 규모로 커지는 경우.
