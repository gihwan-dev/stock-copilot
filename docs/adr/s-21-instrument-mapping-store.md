# ADR 초안: 종목 내부 ID 대응 SQLite 저장소 (S-21)

- 상태: 제안 (초안)
- 날짜: 2026-11-23
- 결정한 사람: 구현 담당 초안(사용자·메인 확인 대기)
- 관련 작업: S-21 (연관 D-52, T-21, S-15, D-55)
- 반영한 문서: `crates/adapters/store-sqlite/migrations/0008_instrument_mappings.sql`, `crates/adapters/store-sqlite/schema.snapshot.sql`, `crates/adapters/store-sqlite/migrations.lock`

## 맥락

D-52([docs/adr/0145-instrument-id-mapping.md](0145-instrument-id-mapping.md))는 종목의 시장·심볼·ISIN(`InstrumentSource`)을 앱 내부 불투명 식별자(`InstrumentId`)로 대응시키는 도메인 규칙(`InstrumentMappings`)과 비동기 포트(`InstrumentMappingRepository`)를 정의했다. 작업 S-21은 이 포트를 `crates/adapters/store-sqlite`에 구현하면서 다음 제약을 모두 만족해야 한다.

- 발급(`ensure`)과 폐지(`retire`)는 한 SQLite 트랜잭션 안에서 원자적으로 처리되며, 두 연결이 동시에 경쟁할 때도 단일 승자·폐지 불변성·재활성화 금지를 지킨다.
- DB 열기(`Database::open`)와 매 조회·갱신 복원 시 정확 중복, 원본 겹침(같은 `(market, upper(symbol))` 또는 `upper(isin)`), 서로 다른 원본이 같은 `InstrumentId`를 공유하는 행, 도메인 문법 위반 값을 거부하고 닫힘 실패(`SystemFailure::RepositoryIntegrity`)한다.
- 폐지된 대응의 이력은 삭제·덮어쓰기·재활성화 없이 보존하고, 활성 대응과 전체 이력의 고유 제약을 유지한다.
- `STRICT` 테이블, 왕복(재시작 포함), 깨진 값 거부 테스트를 갖추며, 가짜 구현(`MemoryInstrumentMappingRepository`)과 SQLite 구현(`InstrumentMappingStore`)이 동일한 포트 계약 테스트를 통과한다.
- DB에 저장된 값만으로 종목 마스터 조회를 거친 도메인 타입(`MasterInstrument`, 주문 해결 타입 등)을 만들지 않는다([docs/adr/0059-master-instrument.md](0059-master-instrument.md), [docs/adr/0104-holdings-snapshot-store.md](0104-holdings-snapshot-store.md)).
- 문서에 확정되지 않은 정책 선택지는 임의로 확정하지 않고 아래 "정의 대기"에 기록한다.

## 스키마와 제약

마이그레이션 `0008_instrument_mappings.sql`이 생성하는 객체와 제약은 다음과 같다.

| 대상 | 종류 | 정의 | 역할 |
|---|---|---|---|
| `instrument_mappings` | `STRICT` 테이블 | `instrument_id TEXT PRIMARY KEY`, `market TEXT NOT NULL`, `symbol TEXT NOT NULL`, `isin TEXT NOT NULL`, `status TEXT NOT NULL`, `UNIQUE (market, symbol, isin)` | 내부 ID 고유성, 원본 3-튜플 고유성, BLOB 등 타입 불일치 차단 |
| `instrument_id` `CHECK` | 열 제약 | `instr(instrument_id, char(0)) = 0 AND length(instrument_id) BETWEEN 1 AND 64 AND instrument_id NOT GLOB '*[^!-~]*'` | `domain::InstrumentId` 문법(ASCII 가시문자 1~64자, NUL 금지) 일치 |
| `market` `CHECK` | 열 제약 | `market IN ('KR', 'US')` | `domain::KnownMarket` 열거형 일치 |
| `symbol` `CHECK` | 열 제약 | `instr(symbol, char(0)) = 0 AND length(symbol) BETWEEN 1 AND 32 AND symbol GLOB '[A-Za-z0-9]*' AND symbol NOT GLOB '*[^A-Za-z0-9.-]*'` | `domain::Symbol` 문법(영숫자 시작, 영숫자·`.`·`-` 1~32자, NUL 금지) 일치 |
| `isin` `CHECK` | 열 제약 | `instr(isin, char(0)) = 0 AND length(isin) BETWEEN 1 AND 64 AND isin NOT GLOB '*[^!-~]*'` | `domain::IsinCode` 문법(ASCII 가시문자 1~64자, NUL 금지) 일치 |
| `status` `CHECK` | 열 제약 | `status IN ('active', 'retired')` | `domain::InstrumentMappingStatus` 열거형 일치 |
| `instrument_mappings_symbol_key` | `UNIQUE INDEX` | `(market, upper(symbol))` | 같은 시장 내 대소문자 변형 포함 심볼 중복·겹침 차단(활성·폐지 공통) |
| `instrument_mappings_isin_key` | `UNIQUE INDEX` | `(upper(isin))` | 전 시장 대소문자 변형 포함 ISIN 중복·겹침 차단(활성·폐지 공통) |
| `instrument_mappings_no_delete` | `BEFORE DELETE` 트리거 | 모든 삭제에 `RAISE(ABORT, ...)` | 활성·폐지 이력 삭제 차단 |
| `instrument_mappings_retire_only` | `BEFORE UPDATE` 트리거 | `OLD.status != 'active'`, `NEW.status != 'retired'`, 또는 식별 열 변경 시 `RAISE(ABORT, ...)` | `active -> retired` 단일 전이만 허용, 재활성화·재갱신·식별자 변경 차단 |
| `instrument_mappings_no_replace` | `BEFORE INSERT` 트리거 | 기존 `instrument_id`, `(market, upper(symbol))`, `upper(isin)` 충돌 시 `RAISE(ABORT, ...)` | `INSERT OR REPLACE`가 내부 삭제 후 재삽입으로 이력을 덮어쓰는 경로 차단 |
| `instrument_mappings_insert_active_only` | `BEFORE INSERT` 트리거 | `NEW.status != 'active'`이면 `RAISE(ABORT, ...)` | `retired` 상태로 직접 삽입해 수명 주기를 우회하는 경로 차단 |

## 상황 → 결과

| 연산 | 상황 | 결과 | 비고 |
|---|---|---|---|
| `Database::open` | 정상 빈 DB 또는 정상 활성·폐지 행 | `Ok(Database)` | 마이그레이션 `0008` 적용 및 `validate_on_open`(스키마 객체 검증 + O(n log n) 원장 검증) 통과 |
| `Database::open` | 정확 중복, `(market, upper(symbol))`·`upper(isin)` 겹침, 서로 다른 원본의 `InstrumentId` 공유, 도메인 문법 위반 행, 또는 테이블·인덱스·트리거 DDL 변조 | `Err(SystemPortError { port: Repository, failure: RepositoryIntegrity })` | 열기 단계에서 스키마 객체와 전체 원장 불변식을 모두 닫힘 검증 |
| `resolve(source)` | 정확 일치 활성 행 존재 | `Ok(InstrumentId)` | |
| `resolve(source)` | 정확 일치 폐지 행 존재 | `Err(MappingStoreError::Mapping(Retired))` | |
| `resolve(source)` | 미등록 원본 | `Err(MappingStoreError::Mapping(Missing))` | |
| `resolve(source)` | 대소문자 변형 또는 심볼/ISIN 일부 겹침 | `Err(MappingStoreError::Mapping(IdentityUncertain))` | 활성·폐지 행 모두에 대해 겹침 검출 |
| `lookup(source)` | 정확 일치 활성 또는 폐지 행 존재 | `Ok(InstrumentMappingRecord)` | 원본·ID·상태를 그대로 반환 |
| `lookup(source)` | 미등록 또는 겹침 | `Err(MappingStoreError::Mapping(Missing \| IdentityUncertain))` | |
| `for_id(id)` | 해당 ID 행 존재(활성·폐지 모두) | `Ok(Vec<InstrumentMappingRecord>)` | |
| `for_id(id)` | 해당 ID 없음 | `Err(MappingStoreError::Mapping(Missing))` | |
| `ensure(source, candidate)` | 미등록 원본, 미사용 후보 ID | `Ok(MappingAssignment::Issued(candidate))` | `BEGIN IMMEDIATE` 트랜잭션 내 `INSERT` 1행 확인 |
| `ensure(source, candidate)` | 이미 활성인 동일 원본(후보 ID가 달라도) | `Ok(MappingAssignment::Existing(existing_id))` | 기존 ID 유지, 새 행 미삽입 |
| `ensure(source, candidate)` | 이미 폐지된 동일 원본 | `Err(MappingStoreError::Mapping(Retired))` | 재활성화 금지 |
| `ensure(source, candidate)` | 심볼·ISIN·대소문자 겹침 | `Err(MappingStoreError::Mapping(IdentityUncertain))` | |
| `ensure(source, candidate)` | 다른 원본이 이미 사용 중인 후보 ID(활성·폐지 모두) | `Err(MappingStoreError::Mapping(IdAlreadyUsed))` | |
| `retire(source)` | 활성 동일 원본 | `Ok(id)` | `BEGIN IMMEDIATE` 트랜잭션 내 `UPDATE ... status = 'retired'` 1행 확인 |
| `retire(source)` | 이미 폐지된 동일 원본 | `Ok(id)` | 멱등, 추가 쓰기 없음 |
| `retire(source)` | 미등록 또는 겹침 | `Err(MappingStoreError::Mapping(Missing \| IdentityUncertain))` | |
| 두 연결 동시 `ensure` / `retire` | 동일 원본·동일 후보 ID·겹침 원본·`ensure`와 `retire` 경쟁 | 한쪽만 승자(`Issued` 또는 선행 `retire`), 반대쪽은 일관된 계약 결과(`Existing`/`IdAlreadyUsed`/`IdentityUncertain`/`Retired`) | `BEGIN IMMEDIATE`로 직렬화 |
| 모든 연산 | 잠금 대기 시간 초과(`SQLITE_BUSY` / `SQLITE_LOCKED`) | `Err(MappingStoreError::Storage(SystemPortError { port: Repository, failure: Unavailable(RetryDelay::Unknown) }))` | 쓰기 롤백, 부분 상태 미노출 |
| `ensure` / `retire` | 쓰기 트리거 실패 또는 영향 행 수 `!= 1` (`RAISE(IGNORE)` 포함) | `Err(MappingStoreError::Storage(SystemPortError { port: Repository, failure: RepositoryIntegrity }))` | 트랜잭션 롤백, DB 상태 불변 |
| 모든 연산 | 조회·갱신 대상 키와 일치하는 행에 깨진 값·중복·겹침·공유 ID 발생 | `Err(MappingStoreError::Storage(SystemPortError { port: Repository, failure: RepositoryIntegrity }))` | 점조회 후보 행 복원 시 도메인 규칙으로 재검증(전체 원장·스키마 변조는 `Database::open`에서 차단) |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| `STRICT` 단일 테이블 `instrument_mappings`에 도메인 문법 `CHECK`, 대소문자 무시 `UNIQUE INDEX`, 4개 보호 트리거를 둔다 | D-52의 전역 단일 대응·폐지 이력 보존·대소문자 무시 겹침 차단 규칙을 SQLite 계층에서도 1차 방어하고, `DELETE`/`UPDATE`/`INSERT OR REPLACE`/`UPSERT`/직접 `retired` 삽입을 원천 봉쇄한다 | 앱 코드 검사만으로 막고 DB 제약·트리거를 생략하는 안(직접 SQL·외부 도구 변조에 취약); 부분 인덱스(`WHERE status = 'active'`)만 두는 안(폐지된 원본의 심볼·ISIN 재사용 및 `IdAlreadyUsed`를 DB 계층에서 막지 못함) | `docs/adr/0145-instrument-id-mapping.md`, `docs/tasks.md` S-21 |
| `Database::open`(`migration::apply_guarded`)에서 `instrument_mappings`의 DDL 객체 7종(테이블·인덱스 2·트리거 4) 일치와 전체 원장 무결성(`BTreeSet` + `domain::instrument_source_conflicts`)을 O(n log n)으로 1회 검증하고, 개별 포트 연산은 SQLite 인덱스 기반 점조회(`BY_SOURCE`, `BY_SOURCE_OR_ID`, `BY_ID`)로 일치 후보 행만 `domain::InstrumentMappings`에 복원해 판정한다 | 열기 시점에 스키마 변조와 원장 전체 중복·겹침·공유 ID·깨진 값을 O(n log n)으로 빠짐없이 거부하고, T-21 `/stocks/all`의 수천~1만 종목 연속 조회·발급에서도 SQLite `MULTI-INDEX OR` 인덱스를 타 O(log n)/호출로 `PERF-1`·`PERF-4`를 지킨다. 후보 행 판정은 여전히 `domain::InstrumentMappings` 단일 원천을 거친다 | 매 호출마다 전체 원장을 `InstrumentMappings::ensure` O(n²)로 재복원하는 안(1차 리뷰·비판에서 n=5,000 기준 Release 평균 264.5ms/호출, T-21 전체 ~22분으로 `PERF-4` 위반이 실측됨); 열기 시 `instrument_mappings` 인덱스·트리거 검증을 생략하는 안(변조 DB에서 DB 1차 방어·인덱스 상실) | `docs/tasks.md` S-21, `docs/design.md` PERF-1·PERF-4, `docs/architecture.md`, `AGENTS.md` 7번 |
| `ensure`와 `retire`는 `Database::write`(`BEGIN IMMEDIATE`) 단일 트랜잭션에서 복원·판정·갱신을 수행한다 | WAL 모드에서 두 연결이 동시에 읽은 뒤 쓰기로 승격할 때 발생하는 즉시 `SQLITE_BUSY_SNAPSHOT` 교착을 막고, 두 연결 경쟁을 SQLite 쓰기 잠금으로 직렬화한다 | `DEFERRED` 트랜잭션 사용(동시 읽기→쓰기 승격 교착 위험); 읽기와 쓰기를 별도 트랜잭션으로 분리(TOCTOU 경쟁으로 중복 발급·재활성화 위험) | `docs/tasks.md` S-21, `crates/adapters/store-sqlite/src/worker.rs` |
| 저장소는 `InstrumentSource`, `InstrumentId`, `InstrumentMappingRecord`만 입출력하고 `MasterInstrument`나 주문 해결 타입을 만들지 않는다 | ADR 0059·0104는 종목 마스터 조회를 거친 타입의 생성을 마스터 어댑터·도메인 경계로 제한하며, 로컬 DB 캐시·대응표만으로 마스터 검증 완료 상태를 위조해서는 안 된다 | 대응 저장소에서 바로 `MasterInstrument`를 조립해 반환하는 안(ADR 0059·0104 위반) | `docs/adr/0059-master-instrument.md`, `docs/adr/0104-holdings-snapshot-store.md`, `docs/adr/0150-instrument-master.md` |
| `crates/testkit-ports/src/contracts/instrument_mapping.rs`에 공유 계약 테스트를 두고 가짜 구현과 SQLite 구현 양쪽에서 실행한다 | D-52 가짜 구현과 SQLite 구현의 계약 일치를 기계적으로 보장하고, 두 연결 경쟁·중단(미폴링 드롭)·참조 모델 수열까지 동일 기준으로 검증한다 | 각 크레이트에 별도 복제 테스트를 작성하는 안(계약 드리프트 발생) | `docs/tasks.md` S-21, `crates/testkit-ports` |

## 정의 대기 (선택지와 사용자 영향)

문서(`docs/open-issues.md`, `docs/tasks.md`, `docs/adr/0145-instrument-id-mapping.md`, `docs/adr/0150-instrument-master.md`)에 아직 확정되지 않은 항목은 임의로 결정하지 않고 선택지와 사용자 영향을 아래와 같이 정리한다.

| 항목 | 선택지 | 사용자 영향 | 현재 처리 |
|---|---|---|---|
| 1. 독립 ID 발급원(`InstrumentIdIssuer`) 형식과 생성 규칙 (D-55 인계) | A: 불투명 무작위 식별자(UUIDv4/v7 등) / B: 단조 증가 불투명 키(예: 접두어+시퀀스) / C: 저장소 내부 시퀀스 테이블 발급 | 사용자가 직접 보는 값은 아니나 백업 복원·기기 이전·로그 진단 시 식별자 길이·정렬성·예측 가능성에 영향 | `InstrumentMappingRepository::ensure`는 호출자가 전달한 `candidate: &InstrumentId`를 그대로 검증·저장하며 발급원 형식은 강제하지 않음 |
| 2. 기존 테이블(`market_quotes`, `market_candles`, `information_links`, `thesis_notes`)의 기존 `instrument_id` 편입 및 테이블 간 FK 정책 | A: 별도 마이그레이션에서 기존 행을 백필한 뒤 FK 추가 / B: `instrument_mappings`를 마스터 경계 원장으로 유지하고 기존 캐시·메모 테이블은 점진 전환 / C: 기존 캐시 테이블 초기화 후 신규 ID만 저장 | 기존에 저장된 시세 캐시·메모·정보 링크가 마이그레이션 후 유지되는지, 미등록 ID 기록이 차단되는지에 직접 영향 | 마이그레이션 `0008`은 기존 테이블 스키마·데이터를 변경하지 않고 독립 테이블 `instrument_mappings`만 추가 |
| 3. 종목 마스터 권한 세션(`InstrumentMasterAuthority` / `InstrumentMasterSession`)의 토스 실측 근거와 네트워크 대기 중 잠금 범위 (ADR 0150 인계) | A: 마스터 조회는 DB 잠금 밖에서 수행하고 결과 확정 시점에만 짧은 `BEGIN IMMEDIATE` 트랜잭션 사용 + 호출자 측 단일 비행(single-flight) / B: 직렬화된 권한 세션 큐에서 조회와 대응 발급을 순차 실행 | 종목 검색·동기화 응답 지연과 동시 조회 시 대기 시간, 그리고 네트워크 오류 시 DB 쓰기 차단 여부에 영향 | `InstrumentMappingStore`는 순수 DB 트랜잭션만 짧게 수행하며, 마스터 조회·세션 직렬화 계층은 토스 실측 및 후속 작업으로 유지 |
| 4. 사용자 확인된 심볼·ISIN 변경 이력 연결과 단일 `InstrumentId`의 복수 원본 허용 정책 (D-55 인계) | A: 변경 전 원본을 `retired`로 남기고 같은 `InstrumentId`에 새 원본 행을 추가(다대일)하도록 `PRIMARY KEY`를 `(instrument_id, market, symbol, isin)` 등으로 확장 / B: 별도 이력 연결 테이블(`instrument_aliases` 등) 도입 / C: 단일 `InstrumentId`당 단일 원본 유지하고 상위에서 승계 링크 관리 | 티커 변경·액면병합·ISIN 변경 시 기존 메모·보유 내역·거래 기록이 같은 종목으로 이어지는지, 수동 확인 절차가 필요한지에 영향 | 현재 D-52 규칙(`IdAlreadyUsed`: 한 ID당 한 원본)에 맞춰 `instrument_id PRIMARY KEY`와 `restore`의 단일 ID 제약을 유지 |
| 5. 원장 무결성 오류(`RepositoryIntegrity`)와 T-21 최종 마스터 응답 검증 오류의 상위 구분·복구 절차 (ADR 0150 인계) | A: 앱 실행 차단 후 백업 복원·DB 재구축 안내 / B: 대응 원장 읽기 전용 진단 모드 제공 / C: 마스터 응답 불일치만 해당 종목 거래 차단으로 격리하고 DB 무결성 오류는 전체 중단 | DB 손상 또는 브로커 응답 이상 발생 시 앱이 어디까지 안전하게 멈추고 사용자가 어떤 복구 단계를 밟아야 하는지에 영향 | `store-sqlite`는 원장 손상을 일관되게 `SystemFailure::RepositoryIntegrity`로 보고하며 UI/상위 복구 흐름은 후속 작업으로 남김 |
| 6. 다중 DB 연결 경쟁 시 명시적 `busy_timeout` 값과 재시도 정책 | A: rusqlite 기본값(5초) 유지 / B: 연결 열기 시 명시적 `PRAGMA busy_timeout` 고정(예: 5000ms) + 상위 재시도 정책 분리 / C: 짧은 타임아웃 + 지수 백오프 재시도 | 다중 창·백그라운드 동기화와 사용자 조작이 겹칠 때 일시 대기 시간과 "일시 사용 불가" 오류 빈도에 영향 | `connection.rs` 기존 설정(rusqlite 기본 5초 `busy_timeout`)을 변경하지 않고, 초과 시 `SystemFailure::Unavailable(RetryDelay::Unknown)` 반환 |

## 기존 테스트 변경 (기계적 고정 개수 갱신)

마이그레이션 `0008` 추가에 따라 전체 마이그레이션 수 및 `sqlite_master` 스키마 객체 수를 고정 상수로 검증하는 기존 테스트만 기계적으로 갱신했다(단언 삭제·약화 없음).

| 파일 | 테스트/상수 | 전 → 후 | `xtask test-diff` 지적 문자열 |
|---|---|---|---|
| `crates/adapters/store-sqlite/src/migration_tests.rs` | `fresh_open_records_clock_and_reopen_preserves_ledger` (`LEDGER_COUNT`) | `7` → `8` | `store-sqlite migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger: test-diff:modified:store-sqlite migration_tests::fresh_open_records_clock_and_reopen_preserves_ledger` |
| `crates/adapters/store-sqlite/src/tests.rs` | `raw_key_is_independently_readable_and_debug_is_closed` (스키마 객체 수) | `42` → `51` (테이블 1 + 명시 인덱스 2 + 자동 인덱스 2 + 트리거 4 = +9) | `store-sqlite tests::raw_key_is_independently_readable_and_debug_is_closed: test-diff:modified:store-sqlite tests::raw_key_is_independently_readable_and_debug_is_closed` |
| `crates/adapters/store-sqlite/src/sql/holdings_tests.rs` | `GAP` / `FUTURE` (미래 마이그레이션 번호 상수) | `0008_...` → `0009_...` (`8` → `9`) | (`test-diff` 미지적; `migration_tests`의 격차·미래 버전 검사 입력 상수) |

## 실측

- 도구: `cargo 1.99.1`, `cargo-nextest 0.9.151`, `node v24.22.0`, `just 1.59.0`
- 커버리지 (`cargo llvm-cov nextest --locked -p store-sqlite --summary-only`):
  - 1차: `instrument_mapping.rs` 줄 172/172(100.00%), 함수 25/25(100.00%), 영역 263/271(97.05%)
  - 2차: `instrument_mapping.rs` 줄 267/269(99.26%), 함수 36/36(100.00%) (미실행 2줄은 `query_map(...)?`의 닫는 오류 경로)
- Release `n=5,000` 성능 실측 (격리 사본, SQLCipher 암호화 DB, `cargo test --locked --release`):
  - 1차(매 호출 전체 원장 `InstrumentMappings::ensure` O(n²)): 열기 `190.1ms`, `resolve` 평균 `264.5ms`(5,000회 추정 `1,322.7s`), `ensure` 신규 `339.4ms`
  - 2차(열기 `validate_schema` + O(n log n) `validate_ledger`, 포트 연산 인덱스 점조회): 열기 `28.39ms`(6.7배 단축), `resolve` 5,000회 `415.76ms`(평균 `0.0832ms`, 3,179배 단축), `ensure` 기존 500회 `33.97ms`, `ensure` 신규 `0.36ms`(943배 단축)
- 손 변형 (격리 사본 `target/S-21/work`에서 실제 실행 후 사본 삭제): 1차 26건(M01~M26) + 2차 12건(M27~M38) = 총 38건 컴파일·실행, 38건 검출(생존 0건).
  - 1차 초기 실행에서 M20·M21(`symbol_key`·`isin_key` 고유 인덱스 제거)이 `no_replace` 트리거에 가려 생존했으나, 보호 트리거 제거 후 인덱스 단독 검증 테스트(`sqlite_unique_indexes_enforce_case_insensitive_keys_without_triggers`)를 추가해 26/26 검출을 확인했다.
  - 2차 초기 실행에서 M27(`validate_schema`의 본문 비교 생략)과 M33(`validate_on_open`의 `validate_ledger` 생략이 `fts_schema::validate`의 `PRAGMA quick_check`에 가려짐)이 생존했으나, 같은 객체 수(7개)로 트리거 본문만 약화한 `REPLACE_WEAK_NO_DELETE_TRIGGER` 케이스와 `validate_on_open` 직접 단언을 추가해 12/12(누적 38/38) 검출을 확인했다.

| ID | 대상·변형 내용 | 결과 | 검출 테스트(대표) |
|---|---|---|---|
| M01 | `market_tag`: `Kr => "US"` 오매핑 | 검출 | `sqlite_store_passes_shared_collision_contract` 외 |
| M02 | `decode_market`: `"KR" => Us` 오매핑 | 검출 | `sqlite_store_passes_shared_concurrency_contracts_on_single_connection` 외 |
| M03 | `decode_market`: 미지 시장값 `Ok(Kr)` 허용 | 검출 | `open_and_operations_reject_corrupted_column_values` |
| M04 | `decode_status`: `active`/`retired` 뒤바꿈 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M05 | `decode_status`: 미지 상태값 `Ok(Active)` 허용 | 검출 | `open_and_operations_reject_corrupted_column_values` |
| M06 | `restore`: 정확 중복(`Existing`) 묵인 | 검출 | `open_and_operations_fail_on_duplicate_overlap_or_shared_id` |
| M07 | `restore`: 원본 겹침·공유 ID 오류 묵인 | 검출 | `open_and_operations_fail_on_duplicate_overlap_or_shared_id` |
| M08 | `restore`: `retired` 행의 폐지 상태 미복원 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M09 | `ensure_mapping`: 신규 발급 시 `INSERT` 생략 | 검출 | `sqlite_store_passes_shared_collision_contract` 외 |
| M10 | `ensure_mapping`: `inserted != 1` 검증 제거 | 검출 | `injected_write_failures_and_zero_row_effects_roll_back_cleanly` |
| M11 | `ensure_mapping`: 신규 행을 `retired`로 삽입 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M12 | `retire_mapping`: 이미 폐지된 원본 멱등성 제거 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M13 | `retire_mapping`: 활성 행 `UPDATE` 생략 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M14 | `retire_mapping`: `updated != 1` 검증 제거 | 검출 | `injected_write_failures_and_zero_row_effects_roll_back_cleanly` |
| M15 | `map_result`: 도메인 오류를 `Missing`으로 뭉갬 | 검출 | `sqlite_store_passes_shared_collision_contract` 외 |
| M16 | `resolve`: 폐지 행도 `lookup`으로 ID 반환 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M17 | `for_id`: 미존재 시 빈 벡터 반환 | 검출 | `sqlite_store_passes_shared_lifecycle_contract` 외 |
| M18 | `migration::apply_guarded`: 열기 시점 원장 검증 제거 | 검출 | `open_and_operations_fail_on_duplicate_overlap_or_shared_id` |
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
| M29 | `validate_ledger`: `seen_ids` 중복 검사 생략 | 검출 | `validate_ledger_accepts_valid_unindexed_rows_and_rejects_duplicate_overlap_or_shared_id` |
| M30 | `validate_ledger`: `seen_sources` 정확 중복 검사 생략 | 검출 | `validate_ledger_accepts_valid_unindexed_rows_and_rejects_duplicate_overlap_or_shared_id` |
| M31 | `validate_ledger`: `instrument_source_conflicts` 겹침 거부 생략 | 검출 | `validate_ledger_accepts_valid_unindexed_rows_and_rejects_duplicate_overlap_or_shared_id` |
| M32 | `validate_on_open`: `validate_schema` 호출 생략 | 검출 | `open_rejects_tampered_mapping_schema_objects` |
| M33 | `validate_on_open`: `validate_ledger` 호출 생략 | 검출 | `open_and_operations_reject_corrupted_column_values` |
| M34 | `sql::BY_SOURCE`: `upper(isin)` 조건 누락 | 검출 | `sqlite_store_passes_shared_overlap_and_case_contract` 외 |
| M35 | `sql::BY_SOURCE_OR_ID`: `instrument_id` 조건 누락 | 검출 | `sqlite_store_passes_shared_collision_contract` 외 |
| M36 | `sql::BY_SOURCE`: `upper(symbol)` 대소문자 정규화 제거 | 검출 | `sqlite_store_passes_shared_overlap_and_case_contract` 외 |
| M37 | `migration::apply_guarded`: `validate_on_open` 호출 제거 | 검출 | `open_rejects_tampered_mapping_schema_objects` 외 |
| M38 | `testkit-ports`: `concurrent_overlapping_sources` 승자 0건 fallthrough 재주입 | 검출 | `overlapping_race_contract_rejects_double_uncertain_without_winner` 외 |

## 결과

- `crates/adapters/store-sqlite`에 `InstrumentMappingStore`와 마이그레이션 `0008_instrument_mappings.sql`이 추가된다.
- `crates/testkit-ports/src/contracts/instrument_mapping.rs`의 공유 계약 테스트가 `MemoryInstrumentMappingRepository`와 `InstrumentMappingStore` 모두의 회귀 기준선이 된다.
- 후속 작업: D-55(독립 ID 발급원·변경 이력), S-15(종목 마스터·대응 연결), T-21 후속(토스 실측 및 권한 세션 직렬화).
- 바뀌면 다시 볼 조건: D-55가 단일 `InstrumentId`에 복수 원본(변경 이력)을 허용하거나, 기존 테이블에 `instrument_mappings(instrument_id)` 외래 키를 도입하거나, 열기 시점 전체 원장 O(n log n) 검증 비용이 `PERF-1`(2초)에 근접할 만큼 수십만 행 규모로 커지는 경우.
