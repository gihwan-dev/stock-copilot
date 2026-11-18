# ADR 0138: S-05 뉴스·공시 저장소와 검색

- 상태: 확정 (2026-11-18)


## 결정

뉴스는 출처와 정규화 URL, 공시는 공시 출처와 접수 번호를 식별자로 쓴다. 같은 식별자를 다시 저장하면 한 항목을 갱신하고 원문과 색인을 한 트랜잭션에서 바꾼다. 제목과 본문은 도메인 값으로 받은 문자열을 그대로 저장하며, 읽을 때 도메인 생성자로 다시 검증한다. 검색과 색인 복구도 잘못된 원문을 발견하면 실패한다.

종목 연결은 `InformationLinks::set_instruments`에 항목 식별자와 종목 ID의 집합을 별도 인자로 넘겨 저장한다. 같은 ID는 한 번만 저장하고 기존 연결 집합을 교체한다. 수집 전에 연결을 저장할 수 있지만 항목이 저장되기 전에는 조회 결과에 나오지 않는다. 뉴스 도메인 타입을 바꾸지 않는다. 공시 저장·식별자 조회·종목별 조회와 뉴스·공시 통합 검색을 위한 application 포트를 둔다.

인접한 유니코드 코드 포인트 두 개를 앱에서 숫자로 인코딩해 FTS5의 토큰으로 넣는다. 따라서 한글, 영문, 숫자, 공백과 기호도 같은 방식으로 검색한다. 영문 검색에서 대소문자만 다르다는 이유로 결과가 빠지지 않도록 색인과 질의의 ASCII 영문만 소문자로 비교한다. 제목과 본문 원문은 바꾸지 않는다. 도메인에 NFC 변환 규칙이 없으므로 NFC로 변환하지 않으며 ASCII 이외 문자의 대소문자는 접지 않는다. 두 글자 이상은 바이그램 구문의 연속 일치와 원문 확인을 사용하고, 한 글자는 저장 원문을 검사한다. 빈 질의는 빈 결과다. 결과 상한은 1~100건의 닫힌 값으로 받는다. `rebuild_index`는 원문 전체에서 색인을 같은 트랜잭션으로 다시 만든다. 이 작업이 아직 배포되기 전의 개발 데이터베이스에서 이전 방식으로 만든 색인은 `rebuild_index`로 갱신해야 한다.

0005 마이그레이션은 원문과 항목·종목 연결을 STRICT 테이블로 저장하고 FTS5 가상 테이블을 추가한다. 실제 SQLCipher 번들에서 SQLite 3.54.1, SQLCipher 4.20.0, `ENABLE_FTS5=1`과 FTS5 생성·검색·삭제를 확인했다. FTS5 및 SQLite가 만드는 그림자 테이블은 STRICT를 지원하지 않는다. 스키마 검사는 마이그레이션에 선언된 FTS5 이름, SQLite가 보고한 객체 유형과 정확한 그림자 이름·생성 형태를 대조한다. 가상 테이블이 있을 때 정적 `PRAGMA quick_check(1)`로 무결성을 검사한다. 실제 번들에서 그림자 내부 데이터가 손상된 FTS5를 이 검사가 거부함을 확인했다. 마이그레이션이 그림자 이름의 일반 테이블을 직접 만들면 내용이 복사되어 무결성 검사를 통과하더라도 거부한다. 그 밖의 비STRICT 테이블과 FTS5가 아닌 가상 테이블은 거부한다. 선언한 FTS5 이름마다 별도 SQL을 등록할 필요는 없다.

SQLite는 일반 테이블을 삭제 후 같은 그림자 이름과 같은 스키마·내용으로 재생성해도 `pragma_table_list`에 그림자로 표시한다. 따라서 마이그레이션 밖에서 완전히 같은 형태와 내용을 재구성한 경우 SQLite 메타데이터만으로 생성 경로를 입증할 수 없다. 이 검사는 스키마 및 색인 일관성 검증이며 데이터베이스 파일의 변경 주체를 인증하지 않는다.

`quick_check`는 FTS만이 아니라 데이터베이스 전체를 검사한다. 이 추가 검사의 큰 데이터베이스 시작 시간은 측정하지 않았다.

## 인계

- N-09: 수집 항목의 중복 제거·사건 묶기와 실제 종목 연결을 조립한다.
- N-10: 종목 매칭 결과를 항목·종목 연결 포트에 전달한다.
- N-11·N-13: 수집 스케줄과 검색 도구를 원문 저장·조회 포트에 연결한다.

## 검증

SQLCipher 번들의 FTS5 실측, 한글 조사·두 글자·한 글자·영문·숫자·기호·분해 한글 검색, 원문 색인 재구축, 출처별 중복과 종목 격리, 상한, 실패 롤백과 손상 원문 거부를 검사한다. 비STRICT 일반 테이블, FTS5가 아닌 가상 테이블, 그림자 이름을 모방한 일반 테이블은 별도 위반 예제로 검사한다. 독립 리뷰 결과는 실행 후 기록한다.

## 손 변형

1차 리뷰 전 별도의 mktemp 소스와 빌드 디렉터리에서 각 변형을 따로 컴파일·실행했다. 실행한 27개 중 26개를 테스트가 검출했고 23번은 생존했다. 34번은 교체 대상이 두 곳이라 실행하지 않았으며 집계에 넣지 않았다. 1차 비판에서 23번의 동등성 판단이 잘못됐음을 실제 SQLCipher 번들과 실행 가능한 변형으로 확인했다. 같은 rowid에 다시 삽입해도 이전 토큰이 남으며, 기존 테스트의 예전 질의가 이를 드러내지 못했다. 두 글자 이상인 이전 검색어의 소멸을 검사하는 회귀를 추가했다.

1차 리뷰 수정 소스와 테스트를 같은 방식의 격리된 사본에서 재검증했다. 기존 27개와 ASCII 대소문자 비교 변형 2개를 포함한 컴파일 가능한 29개 모두 테스트가 검출했다. 34번은 계속 미실행이며 집계에 넣지 않았다. 실행 후 소스와 테스트의 바이트 일치를 확인했다.

| 번호 | 변형 | 결과 |
|---|---|---|
| 21 | 바이그램이 한 글자 건너뛰기 | `bigram_search_preserves_raw_text_and_rebuilds_identically` 검출 |
| 22 | 본문을 색인에서 누락 | `bigram_search_preserves_raw_text_and_rebuilds_identically` 검출 |
| 23 | 갱신 때 기존 색인 삭제 생략 | 1차 리뷰 전 생존. 수정 후 `updating_an_item_removes_every_old_bigram_from_search` 검출 |
| 24 | 빈 검색어 판정 반전 | `bigram_search_preserves_raw_text_and_rebuilds_identically` 검출 |
| 25 | 두 글자에 한 글자 검색 경로 적용 | `bigram_search_preserves_raw_text_and_rebuilds_identically` 검출 |
| 26 | 한 글자 검색 결과를 한 건으로 제한 | `disclosure_identity_links_and_search_limit_hold_across_categories` 검출 |
| 27 | FTS 검색 상한을 한 건 초과 | `disclosure_identity_links_and_search_limit_hold_across_categories` 검출 |
| 28 | 종목 연결의 출처 조건 제거 | `disclosure_identity_links_and_search_limit_hold_across_categories` 검출 |
| 29 | DART 출처를 KIND로 저장 | `disclosure_identity_links_and_search_limit_hold_across_categories` 검출 |
| 30 | 뉴스 식별자 검증 반전 | `bigram_search_preserves_raw_text_and_rebuilds_identically` 검출 |
| 31 | 검색 바이그램 구문을 두 번 붙임 | `bigram_search_preserves_raw_text_and_rebuilds_identically` 검출 |
| 32 | 검색 결과 원문 검증 반전 | `bigram_search_preserves_raw_text_and_rebuilds_identically` 검출 |
| 33 | 제목과 본문 모두 검색어 포함 요구 | `bigram_search_preserves_raw_text_and_rebuilds_identically` 검출 |
| 35 | 종목 연결 교체에서 기존 연결 삭제 생략 | `replacing_and_clearing_links_changes_only_the_selected_item` 검출 |
| 36 | 색인 재구축에서 기존 색인 삭제 생략 | `bigram_search_preserves_raw_text_and_rebuilds_identically` 검출 (테스트 보강 전 생존) |
| 37 | 검색 상한의 최댓값 축소 | `search_limit_rejects_values_outside_its_closed_range` 검출 |
| 38 | 검색 상한의 최솟값 증가 | `search_limit_rejects_values_outside_its_closed_range` 검출 |
| 39 | 연합뉴스를 한국경제로 저장 | `corrupt_news_is_rejected_without_exposing_an_unrelated_instrument` 검출 |
| 40 | 원문 본문 저장 생략 | `bigram_search_preserves_raw_text_and_rebuilds_identically` 검출 |
| 41 | 항목 저장 때 색인 삽입 생략 | `bigram_search_preserves_raw_text_and_rebuilds_identically` 검출 |
| 42 | 비STRICT 일반 테이블 허용 | `ordinary_nonstrict_table_is_still_rejected` 검출 (테스트 보강 전 생존) |
| 43 | 허용 가상 테이블 모듈을 FTS4로 변경 | `declared_fts5_and_actual_shadow_tables_are_accepted` 검출 |
| 44 | 그림자 이름 검사에서 data 제외 | `explicitly_created_shadow_lookalike_with_copied_contents_is_rejected` 검출 |
| 45 | 정상 idx 그림자 DDL 거부 | `declared_fts5_and_actual_shadow_tables_are_accepted` 검출 |
| 46 | FTS 이름을 하나로 제한 | `declared_fts5_name_and_its_actual_shadow_tables_are_accepted` 검출 |
| 47 | FTS가 있을 때 quick_check 생략 | `quick_check_rejects_damaged_shadow_for_any_declared_fts_name` 검출 |
| 48 | quick_check 결과 판정 반전 | `declared_fts5_and_actual_shadow_tables_are_accepted` 검출 |
| 49 | 색인 생성에서 ASCII 대소문자 접기 생략 | `ascii_case_insensitive_search_preserves_original_and_survives_rebuild` 검출 |
| 50 | 원문 일치 확인에서 질의의 ASCII 대소문자 접기 생략 | `ascii_case_insensitive_search_preserves_original_and_survives_rebuild` 검출 |
