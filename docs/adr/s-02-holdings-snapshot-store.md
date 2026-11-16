# S-02 보유 스냅샷 저장소

- 상태: 초안

## 결정

기준은 730e4069f9327da8e7884c845069130d542066f7이다. 다음 번호 0002_holdings_snapshots.sql 하나를 추가한다. 두 STRICT 테이블에 계좌·시각 헤더와 순서가 있는 보유 관측을 나눠 저장한다. 금액과 수량은 TEXT 정규 십진수 CHECK, 금액은 KRW/USD 통화와 함께 보존한다. 모든 읽기는 domain 숫자·심볼·시각·보고 값 생성자로 재검증한다. SQL 및 오류에는 계좌 본문이나 비밀을 담지 않는다.

HoldingsStore는 조립 루트의 Database를 빌리는 구체 어댑터다. 여러 저장소가 같은 Database를 빌려 전용 쓰기 스레드 하나를 공유한다. 저장소 사용이 끝나면 조립 루트가 Database::close(self)를 호출해 종료 결과를 받는다. PortfolioRepository는 이미 application에 있으므로 새 trait를 만들지 않는다. SnapshotHolding은 보유 관측 필드다. DB만으로 마스터 조회 권한을 나타내는 ResolvedOrderInstrument를 만들지 않는다. 이 관측을 현재 마스터와 결합해 Portfolio로 복원하는 일은 A-06 연결 작업에 넘긴다. 현금과 계좌 전체 평가 기록은 이 보유 저장소의 범위 밖이다. 외부 의존 추가는 없다.

계좌·UTC 초·나노초가 동일하면 같은 스냅샷이다. domain의 동등성에 따라 동일한 순간과 수치인 본문 재시도는 성공하고 다른 본문은 RepositoryIntegrity로 닫는다. 재시도는 최초 저장된 오프셋과 십진수 scale 표현을 바꾸지 않는다. 최초 관측의 표현은 별도 왕복·실제 TEXT 단언으로 보존한다. 삽입과 모든 자식 행은 한 트랜잭션이다. 빈 보유도 헤더를 남긴다. 잘못된 통화 조합과 중복 종목은 실패한다.

| 현재 KST 날짜와 자료 KST 날짜의 차 | 보존 |
|---|---|
| 음수부터 29일 | 전부 |
| 30일 이상 | 계좌·KST 날짜별 UTC 시각이 마지막인 하나 |
| 동일 초의 다른 나노초 | 나노초가 큰 것이 마지막 |
| 축소 중 실패 | 헤더와 자식 행 모두 롤백 |

architecture는 일별 대표 시각을 지정하지 않아 마지막 관측으로 정한다. 시각은 Clock에서 한 번 얻고, KST 달력 날짜 차이를 domain에서 계산한다. SQL에서 금액 계산은 하지 않는다. 삭제는 전용 쓰기 스레드의 한 트랜잭션에서 수행하며 FK CASCADE로 자식도 지운다. J-13이 반복 호출한다. 미래 날짜는 건드리지 않는다.

## 검사와 절차

프로젝트 신뢰·훅 정의 신뢰·현재 세션 적용은 확인 불가다. 시작 pnpm frozen 설치와 lefthook 설치는 성공했다. 테스트 작성 전에 구현 파일을 먼저 작성한 절차상 누락이 있어 회귀 우선 실행을 주장하지 않는다. 최초 신규 테스트 실행은 6 통과·KST fixture 기준 시각 오류 1 실패였고 독립 UTC 변환으로 fixture를 정정했다.

기존 마이그레이션 실행기 테스트는 제품 목록이 늘어도 원래 한 파일에서 시작하는 실험이 되도록 준비 부분을 MIGRATIONS[..1] 적용으로 고정한다. 단언은 유지한다. 실제 제품 열기의 원장 개수 1→2, sqlite_schema 개수 2→7은 정확한 새 값으로 갱신한다. 기존 부정 fixture는 두 행의 UNIQUE 충돌을 피하면서 같은 손상 조건을 만들도록 바꾼다. 사전 승인 (c), (d)에 해당한다. test-diff 목록은 최종 검사 후 기록한다.

보호 파일은 migrations.lock(0002 해시), schema.snapshot.sql(두 테이블 추가)과 스크립트 생성물만 갱신한다. 규칙 완화와 검사 끄기는 추가하지 않는다.

## 인계

- A-06: 기존 PortfolioRepository와 보유 관측 저장 API 연결. 마스터 확인을 거쳐 종목 타입 복원, 현금·계좌 전체 평가·SyncRecord 저장은 별도 조립.
- J-13: compact_holdings 예약 호출과 incremental_vacuum.
- S-03: 이 작업은 0002를 썼으므로 최신 master에서 다음 번호를 확인한다.
- U-01: 마지막 Database 소유권의 비동기 close 수명 주기 연결.

## 승인된 기존 테스트 변경 17건 (test-diff 검출16건과 Node 준비1건)

| 파일 | 테스트 | 전 → 후 |
|---|---|---|
| harness/claude-hooks/tests/allowlist.test.mjs | open-issues and every git-tracked ADR are protected, a new ADR number is not | 준비 경로 0099-future.md → 9999-future.md; 모든 단언 유지 |
| src/migration_tests.rs | corrupt_ledger_and_future_database_fail_open | 두 원장 행에서 UNIQUE 충돌 없는 BAD_NAME·GAP·FUTURE 준비 값; 실패 단언 유지 |
| src/migration_tests.rs | fresh_open_records_clock_and_reopen_preserves_ledger | 원장 개수 1 → 2 |
| src/review_tests.rs | embedded_sequence_gap_rolls_back_pending_schema | 제품 목록 전체 적용 → 원장 0001만 적용하는 원래 실험 준비; 단언 동일 |
| src/review_tests.rs | snapshot_contains_index_view_trigger_and_tables | 제품 목록 전체 적용 → 원장 0001만 적용하는 원래 실험 준비; 단언 동일 |
| src/review_tests.rs | table_rename_quotes_do_not_change_column_identity | 제품 목록 전체 적용 → 원장 0001만 적용하는 원래 실험 준비; 단언 동일 |
| src/review_tests.rs | third_migration_failure_rolls_back_second_migration_too | 제품 목록 전체 적용 → 원장 0001만 적용하는 원래 실험 준비; 단언 동일 |
| src/review_tests.rs | unsupported_schema_forms_fail_after_sqlite_accepts_them | 제품 목록 전체 적용 → 원장 0001만 적용하는 원래 실험 준비; 단언 동일 |
| src/tests.rs | raw_key_is_independently_readable_and_debug_is_closed | sqlite_schema 정확한 개수 2 → 7 |
| src/third_tests.rs | alter_after_quoted_reference_is_closed | 제품 목록 전체 적용 → 원장 0001만 적용하는 원래 실험 준비; 단언 동일 |
| src/third_tests.rs | damaged_initial_ledger_cannot_be_repaired_by_pending_sql | 제품 목록 전체 적용 → 원장 0001만 적용하는 원래 실험 준비; 단언 동일 |
| src/third_tests.rs | hidden_numeric_column_fails_apply_and_preserves_ledger | 제품 목록 전체 적용 → 원장 0001만 적용하는 원래 실험 준비; 단언 동일 |
| src/third_tests.rs | quoted_body_tokens_cannot_hide_numeric_columns | 제품 목록 전체 적용 → 원장 0001만 적용하는 원래 실험 준비; 단언 동일 |
| src/third_tests.rs | single_quoted_text_markers_do_not_change_column_boundaries | 제품 목록 전체 적용 → 원장 0001만 적용하는 원래 실험 준비; 단언 동일 |
| src/third_tests.rs | successful_sql_cannot_commit_a_damaged_ledger | 제품 목록 전체 적용 → 원장 0001만 적용하는 원래 실험 준비; 단언 동일 |
| src/third_tests.rs | supported_negative_and_division_defaults_are_not_comments | 제품 목록 전체 적용 → 원장 0001만 적용하는 원래 실험 준비; 단언 동일 |
| src/third_tests.rs | valid_data_migration_preserves_distinct_application_times | 제품 목록 전체 적용 → 원장 0001만 적용하는 원래 실험 준비; 단언 동일 |

전체 검사로 승격된 check-fast: 프런트·정책·구조·SQL·마이그레이션·1665 테스트·커버리지 통과. test-diff만 위16건으로 실패. 이 커밋에만 사용자 지시의 LEFTHOOK=0 예외를 쓰며 규칙을 변경하지 않는다. 자동 변이는 test-diff 단계에서 중단되었다.

## 손 변형

임시 사본에서 전후 대조군 각각 신규 7개 통과. 컴파일 가능한 22개 전부 검출, 컴파일불가 1개는 제외했다. 자동 mutants-diff는 사전 승인된 기존 테스트 변경16건을 test-diff가 거부해 변이를 실행하지 못했다. 자동 변이 통과로 표시하지 않는다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | < 30 -> < 29 | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account | 검출 |
| 2 | < 30 -> <= 30 | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account | 검출 |
| 3 | OffsetSeconds::new(32_400) -> OffsetSeconds::new(0) | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account | 검출 |
| 4 | latest.insert((entry.account.clone(), date), entry.id) -> latest.insert((AccountId::new(0), date), entry.id) | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account | 검출 |
| 5 | if stored != incoming -> if stored == incoming | holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 6 | if let Some(id) = previous -> if let Some(id) = previous.filter(&#124;_&#124; false) | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children | 검출 |
| 7 | row.get(0)? -> row.get(1)? | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::empty_and_optional_observations_round_trip, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 8 | AccountId::new(row.get(1)?) -> AccountId::new(row.get(0)?) | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::empty_and_optional_observations_round_trip, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 9 | at: timestamp(row, 2)? -> at: timestamp(row, 1)? | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::empty_and_optional_observations_round_trip, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 10 | row.get::<_, String>(2)? -> row.get::<_, String>(3)? | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::corruption_is_rejected_by_domain_and_decimal_constraint, holdings_tests::empty_and_optional_observations_round_trip, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 11 | row.get::<_, String>(3)? -> row.get::<_, String>(2)? | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::empty_and_optional_observations_round_trip, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 13 | row.get::<_, Option<String>>(5)? -> row.get::<_, Option<String>>(6)? | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 14 | row.get::<_, Option<String>>(6)? -> row.get::<_, Option<String>>(5)? | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 15 | row.get::<_, Option<String>>(7)? -> row.get::<_, Option<String>>(8)? | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 16 | row.get::<_, Option<String>>(8)? -> row.get::<_, Option<String>>(9)? | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 17 | row.get::<_, Option<String>>(9)? -> row.get::<_, Option<String>>(8)? | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 18 | timestamp(row, 10)? -> timestamp(row, 9)? | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::empty_and_optional_observations_round_trip, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 19 | Currency::Krw => "KRW" -> Currency::Krw => "USD" | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::corruption_is_rejected_by_domain_and_decimal_constraint, holdings_tests::empty_and_optional_observations_round_trip, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 20 | Currency::Usd => "USD" -> Currency::Usd => "KRW" | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::corruption_is_rejected_by_domain_and_decimal_constraint, holdings_tests::empty_and_optional_observations_round_trip, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 21 | item.quantity.as_decimal().to_string() -> item.acquisition_price.money().amount().as_decimal().to_string() | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::empty_and_optional_observations_round_trip, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 22 | report_value.profit().amount() -> report_value.valuation().amount() | holdings_tests::compaction_uses_kst_dates_29_30_31_and_last_per_account, holdings_tests::failed_compaction_rolls_back_prior_deletes_and_children, holdings_tests::round_trip_reopen_preserves_every_field_and_duplicate_is_idempotent | 검출 |
| 23 | daily_value.rate().as_decimal() -> Ratio::parse(&NumericInput::try_new("0").expect("input")).expect("rate").as_decimal() |  | 컴파일불가 |

## 1차 리뷰 수정

DB 소유권은 조립 루트에 두고 저장소가 공유 차용한다. 외부 공개 API의 명시적 종료 회귀, 원시 TEXT와 offset 보존 단언, 나노초 역순 삽입·자료 KST 자정 경계, 다른 계좌의 손상 격리 회귀를 추가했다. 조회 SQL은 계좌 조건으로 먼저 거른다. ADR의 승인 목록 누락1행을 보충했다. 저장소가 주문용 종목 권한 타입을 복원하거나 PortfolioRepository를 새로 정의하지 않는 범위는 사용자 지시대로 유지한다. 서명 옵션과 ADR 초안 이름도 사용자 지정 그대로다.

1차 수정 후 검사는 crate81/81·clippy 통과, 전체1669/1669(속성4096·동시실행4), 커버리지1059/1059·위반 없음이다. 정상 훅의 기존 Git 훅 회귀2건 실패와 재실행 경위는 리뷰 기록에 구분했다. test-diff는 기존 승인16건이며 신규 승인 대상은 추가되지 않았다. 수정 소스의 손 변형도 유효22개 전부 재검출, 전후 대조군은11개 통과했다. 조립 루트는 공유 차용한 저장소들의 사용이 끝난 뒤 원래 Database::close를 호출한다.


## 앱 상태 조립과 축소 실패 계약

Database를 소유한 앱 상태의 메서드에서 호출마다 HoldingsStore를 잠시 빌린다. 상태를 Arc로 공유하거나 비동기 작업으로 옮기는 일과 저장소의 짧은 차용은 양립한다. 작업 종료 후 상태에서 Database 소유권을 꺼내 close한다. 차용 저장소 자체에 static 수명을 요구하거나 Database를 leak하지 않는다. 이 조립 패턴은 2차 비판의 Tauri manage 컴파일·tokio spawn 저장·close·재열기 탐침에서 확인했으며 실제 데스크톱 이벤트 검증은 U-01/U-08에 인계한다.

축소는 저장소 전체를 한 트랜잭션으로 다루며 어느 계좌든 손상된 헤더가 있으면 RepositoryIntegrity로 끝나 원본 전부를 보존한다. J-13은 이 실패를 숨기거나 손상 자료를 건너뛰지 않는다. 계좌별 부분 성공은 별도 계약 없이는 추가하지 않는다. 숫자 CHECK의 정규 십진수는 architecture6.3의 문법을 뜻하므로 앞자리0과 음의0을 허용한다. 단일 표준 표기로 정규화하지 않으며 최초 scale을 보존한다.

2차 리뷰의 새 제품 지적은 독립 실행 비판으로 모두 기각됐다. 예외 적용의 직접 로그·초기 실패·최종 통과는 docs/reviews/S-02.md에 분리해 기록했다. 프로젝트 신뢰·현재 세션 훅 적용 미확인은 통과 주장으로 바꾸지 않는다.


최신 master520c8df를 수신한 트리의 정상 훅은 전체1695/1695·커버리지1059/1059와 다른 검사를 통과하고 승인test-diff16건만 실패했다. 최종 커밋 예외와 이후 SHA 기준 전체 check를 분리한다. 병합 중 S-02 제품 코드 변경은 없고 기존 master의 새 테스트·규칙은 보존했다. 실패·중단·재실행 경위는 리뷰 기록에 남겼다.


최종 커밋 기준 전체 검사에서 기존 ADR 보호 테스트가 미사용 번호로 가정한0099를 master의 새 ADR0099가 소비한 충돌을 재현했다. 준비 인자의 새 번호만 현재 미사용9999로 바꾸고 단언은 그대로 유지한다. 사용자 사전 승인(d)에 해당하며 최종 사전 승인 변경은 초기16건과 이 Node 준비1건이다. 실제 test-diff 출력은16건이며 이 Node fixture 변경은 출력에 포함되지 않아 직접 diff로 대조했다. 이 fixture 외 제품 코드와 기존 단언의 변경은 없다.

- H-20 인계: 최종 test-diff 출력은 Rust16건이며 이번 Node mjs fixture 한 인자 변경은 검출되지 않았다. 하네스 범위 확대는 이 저장소 작업에서 하지 않고 별도 검토한다.
