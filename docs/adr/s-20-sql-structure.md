# S-20: SQL 검사 구조와 마이그레이션 잠금 절차

- 상태: 확정 (2026-11-25, fin-b 묶음, 사용자 승인 행)
- 관련: ADR 0011, architecture.md 6.3, harness.md 6.8

## 결정

| 항목 | 정한 것 | 버린 대안 |
|---|---|---|
| SQL 위치 | `store-sqlite`의 테스트 아닌 소스에서 SQL은 `src/sql/*.rs`의 `const X: &str = "리터럴";`에만 둔다. 그 파일에는 그 밖의 항목(함수, `use`, 매크로 포함)을 두지 않는다. 그 밖의 소스 파일에서 SQL로 읽히는 문자열은 `sql-structure:outside-module`이다 | 모든 문자열을 SQL 파서로 판정(일반 문구 오탐) |
| 실행 인자 | `execute`(2인자), `execute_batch`, `prepare`, `prepare_cached`, `query_row`·`query_row_and_then`·`query_one`·`query_and_then`(3인자)의 SQL 인자와 `pragma_*`의 이름 인자는 대문자 상수 경로만 받는다. 인자 수로 `Connection`과 `Statement`를 가른다. `pragma_*` 이름은 소문자·밑줄 리터럴도 받는다(SQL이 아니라 이름이다). 매개변수·반복 변수를 넘기는 함수는 `harness/sql-check/forwarders.toml`에 (파일, 함수, 이유)로 적고 그 안에서만 단순 경로 인자를 받는다 | 함수 매개변수의 모든 호출 지점을 추적(복잡) |
| 문장 판정 | `sqlparser 0.64.0`(기본 기능 끔, `std`만) SQLite 방언으로 읽는다. 파서가 모르는 `NOT GLOB`은 `NOT LIKE`로, PRAGMA 값 낱말은 문자열로 바꿔 읽고, FTS5 `CREATE VIRTUAL TABLE`은 낱말만 본다. 마이그레이션 파일과 글자까지 같은 상수는 마이그레이션 상수로 여러 문장을 받고, 그 밖의 상수는 문장 하나이며 `CREATE`·`ALTER`·`DROP`이면 `ddl-outside-migration`이다 | 다른 파서 도입, 렉서만 유지 |
| 금액 열 | 마이그레이션 `CREATE TABLE`에서 `<열> NOT GLOB '*[^0-9.-]*'`을 가진 CHECK(열·테이블 수준)의 열을 금액 열로 뽑는다. CHECK로 못 가리는 열은 `harness/sql-check/money-protected.toml`에 (이름, 이유)로 더한다. 기존 이름 조각 검사(`money-columns.toml`)는 추가 그물로 남긴다 | 이름 조각만 사용 |
| 허용 위치 | 최상위 `SELECT` 목록의 별칭 없는 맨 열, `INSERT` 열 목록, `SET 열 = ?`, `WHERE 열 = ?`(AND·OR·괄호 아래), `ON CONFLICT … SET 열 = excluded.열`, `CREATE TABLE` 열 정의와 CHECK. 문장 전체의 금액 열 낱말 수가 허용 위치 수보다 많으면 `money-placement`다. 그래서 별칭, CTE, 하위 쿼리, 합집합, 함수, 연산자, 비교, `ORDER BY`·`GROUP BY`, 인덱스·뷰·트리거·가상 테이블의 금액 열은 모두 걸린다 | AST 방문자로 위치마다 규칙 작성(`visitor` 기능과 `sqlparser_derive` 추가 필요) |
| 테스트 파일 | `tests.rs`, `*_tests.rs`, `tests/`, `*_tests/` 아래는 구조 검사에서 뺀다. 기존 렉서 검사는 그대로 돈다 | 테스트 SQL도 상수 모듈로 이동 |

## 마이그레이션 잠금 절차

1. 에이전트는 `migrations/NNNN_<이름>.sql`(잠금의 마지막 번호 다음)과 같은 글자의 상수를 `src/sql/<모듈>.rs`에 두고 `migration.rs`의 `MIGRATIONS`에 잇는다. `sql-check`는 이 상수를 마이그레이션으로 보고, 열 정의와 정규 십진수 CHECK에서 금액 열을 다시 뽑는다.
2. 잠금에 오르기 전의 마이그레이션은 고칠 수 있다(ADR 0011). 작업 브랜치에서 여러 번 고쳐도 된다.
3. 잠금에 올리는 때는 그 마이그레이션이 들어간 앱을 처음 배포하기 직전이다. 사람이 `shasum -a 256 migrations/NNNN_*.sql`로 해시를 계산해 `migrations.lock`에 `number`·`file`·`sha256` 항목을 더하고, 사용자 서명 커밋으로 넣는다. 잠금에 올리는 명령은 만들지 않는다(ADR 0011).
4. 잠금에 오른 뒤의 변경은 새 번호의 마이그레이션으로만 한다. `migration-check`가 잠긴 파일의 수정·삭제를 거부한다.

## 남은 빈틈

- `forwarders.toml`에 적힌 함수의 호출 지점이 상수를 넘기는지는 보지 않는다. 목록은 보호 파일이라 사람이 리뷰한다.
- 낱말 수 비교라서, 허용 위치에 금액 열이 하나 남고 같은 이름이 금액 열이 아닌 표의 열로 쓰인 경우는 금액 열로 본다(엄격한 쪽 오탐).
- 동적 SQL(`String` 조립)은 실행 인자 규칙이 막지만, `forwarders.toml` 함수 안에서 경로 변수로 들어오는 값은 믿는다.
