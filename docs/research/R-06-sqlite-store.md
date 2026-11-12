# R-06 SQLite 저장소 조사 결과

- 조사일: 2026-11-11 (모든 출처의 확인 날짜는 이 날짜다)
- 실험 환경: macOS (Darwin 25.5.0, arm64), rustc 1.95.0 (`cargo +1.95.0`), 기본 툴체인은 1.88.0
- 실험 코드: 저장소 밖 임시 폴더에서 만들고 돌렸다. 아래 "실험" 절에 명령, 핵심 코드, 출력을 붙였다.

## 1. 요약

1. 드라이버는 **rusqlite 0.41.x**(정확한 버전 고정)에 **`bundled-sqlcipher`** feature를 쓰는 것을 추천한다. macOS에서 `OPENSSL_DIR`가 없으면 시스템 CommonCrypto로 링크되고(`cipher_provider = commoncrypto`, 링크 대상은 시스템 프레임워크뿐), 별도 설치 없이 빌드된다.
2. 키는 키체인에 둔 **32바이트 난수 원시 키**(`PRAGMA key = "x'…'"`)로 PBKDF2를 건너뛴다. 실측 결과 열기는 0.5ms 대 93ms, 대량 쓰기는 평문보다 약 4배, 읽기는 약 2배 느렸다. 이 앱의 규모에서는 문제가 되지 않는다.
3. 금액·수량은 **STRICT 테이블의 TEXT 십진수 문자열**(토스 스펙의 `^-?\d+(\.\d+)?$`와 같은 형식, CHECK로 강제)에 통화 열을 따로 둔다. 합산·정렬은 SQL에서 하지 않고 Rust의 `rust_decimal`로 한다. NUMERIC 열은 값이 REAL로 바뀌어 정밀도를 잃는 것을 실험으로 확인했다.
4. 한국어 제목 검색은 내장 토크나이저만 쓰는 **앱 측 바이그램 색인**(Rust에서 한글 구간을 2글자로 쪼개 `unicode61` FTS5에 넣는 방식)을 추천한다. 2글자 질의("금리", "전자")와 조사가 붙은 단어("삼성전자가")까지 모두 맞았고, trigram은 2글자 질의를 놓쳤다.
5. 마이그레이션은 직접 만든 작은 실행기와 SHA-256 체크섬 원장으로 처리하고, 감사 로그는 트리거, 연결 단위 권한 콜백(authorizer), 해시 체인, 외부 앵커를 겹쳐 쓴다. `rusqlite_migration`은 `user_version`만 봐서 적용된 마이그레이션을 고쳐도 잡지 못했다(실험 확인).

## 2. 질문별 답

### Q1. Rust SQLite 드라이버 비교

| 항목 | rusqlite | sqlx (SQLite) | diesel | turso (Rust 재작성) |
|---|---|---|---|---|
| 최신 버전 (2026-11-11) | 0.42.0 (오늘 배포), 0.41.1 | 0.9.0 (2026-05-21) | 2.3.14 | 0.8.3 |
| MSRV | 1.89.0 (0.41 이상) | 1.94.0 | 미확인 | 미확인 |
| libsqlite3-sys 범위 | 0.42 → 0.40.0, 0.41.1 → 0.39.1 | `>=0.30.1, <0.38.0` | `>=0.17.2, <0.39.0` | 자체 엔진 (SQLite C 아님) |
| 컴파일 시점 쿼리 검사 | 없음. SQL을 문자열로 다룬다 | `query!` 매크로가 개발 DB에 SQL을 준비해 열·타입·NULL 여부를 검사한다 | 스키마 DSL로 타입 검사 (SQL 문자열이 아님) | 없음 |
| 비동기 | 동기. 전용 스레드나 `spawn_blocking`이 필요하다 | 비동기 (내부적으로 워커 스레드) | 동기 | 비동기 |
| 마이그레이션 | 외부 crate (`rusqlite_migration`, `refinery`) 또는 직접 구현 | 내장 `migrate!`. SHA-384 체크섬을 비교해 다르면 `VersionMismatch` | `diesel_migrations` 2.3.2 | 미확인 |
| 암호화 | `bundled-sqlcipher`, `bundled-sqlcipher-vendored-openssl`, `sqlcipher`(시스템) feature | 직접 feature는 없다. `libsqlite3-sys`에 `bundled-sqlcipher`를 켜면 동작한다 (실험 확인) | libsqlite3-sys feature를 같은 방식으로 켠다 (미실험) | 자체 암호화가 `--experimental-encryption`이고 매뉴얼에 "not production ready"라고 적혀 있다 |
| 기타 | `backup`, `hooks`(authorizer), `serialize` feature | 금액용 `rust_decimal` 매핑을 SQLite에서는 일부러 뺐다 | - | 매뉴얼 제한 목록에 "No triggers" |

확인한 사실:

- **libsqlite3-sys는 `links = "sqlite3"`라서 한 빌드에 버전 하나만 들어간다.** 따라서 rusqlite 0.42(libsqlite3-sys 0.40)와 sqlx 0.9(<0.38)는 같은 작업 공간에 함께 둘 수 없다. 주변 crate도 rusqlite 버전에 묶여 있다. `rusqlite_migration` 2.7.0은 rusqlite 0.41, `refinery-core` 0.10.1은 `<=0.41`, `tokio-rusqlite` 0.8.0은 0.40.1을 요구한다. 오늘 나온 rusqlite 0.42를 바로 쓰면 이 crate들과 맞지 않는다.
- sqlx 0.9 + `libsqlite3-sys 0.37 bundled-sqlcipher` 조합을 실제로 빌드해 열었다. `cipher_version = 4.10.0 community`, SQLite 3.50.4가 나왔다. rusqlite 0.41 조합(SQLCipher 4.19.0, SQLite 3.53.4)보다 두세 단계 오래된 엔진에 묶인다.
- sqlx `query!` 매크로는 **암호화되지 않은 스키마 전용 DB**를 `DATABASE_URL`로 가리키면 동작했다. 없는 열(`no such column: nope`)과 LEFT JOIN 쪽 열의 `Option<String>` 추론을 컴파일 오류로 잡았다(실험 확인). 개발 DB에 실제 데이터가 없어도 되고 스키마만 같으면 된다는 점은 sqlx README에 적혀 있다.
- sqlx는 SQLite에서 `rust_decimal`, `bigdecimal` 매핑을 일부러 뺐다. 소스 문서에 이유가 적혀 있다: "NUMERIC type affinity is unsuitable for storage of high-precision decimal values… avoided at all costs", 그리고 "TEXT"나 "BLOB"에 문자열로 저장하라고 권한다. 이 앱의 금액 설계와 같은 결론이다.
- `tauri-plugin-sql` 2.6.0은 sqlx 0.8을 쓴다. 이 플러그인은 WebView에서 SQL을 부르는 구조여서 "TS는 계산하지 않고 IPC는 `shared/ipc`에서만" 규칙과 맞지 않는다. 쓰지 않는다.

rusqlite에는 컴파일 시점 검사가 없으므로 다음으로 메운다 (권장안, 미실험 부분 있음).

1. 모든 SQL을 `store-sqlite` 안 상수로 모은다.
2. 테스트에서 빈 DB에 마이그레이션을 전부 적용한 뒤 상수마다 `Connection::prepare`를 부른다. 없는 테이블·열과 문법 오류가 잡힌다(실험에서 `prepare`가 `no such column`을 내는 것은 sqlx 경로로만 확인했고, rusqlite 쪽 같은 동작은 SQLite C API의 동작이다).
3. 모든 테이블을 STRICT로 만들어 열 타입 위반을 실행 시점에 바로 오류로 만든다(실험: `cannot store TEXT value in INTEGER column m.minor`).
4. 행 ↔ 도메인 타입 변환은 왕복 테스트로 검사한다(tasks.md S-02 등에 이미 있다).

### Q2. 암호화: SQLCipher와 대안

| 항목 | SQLCipher (rusqlite `bundled-sqlcipher`) | SQLite3MultipleCiphers | Turso 자체 암호화 | SEE (SQLite 공식 유료) | 파일시스템 암호화만 (FileVault) |
|---|---|---|---|---|---|
| 방식 | 페이지 단위 AES-256-CBC + 페이지 HMAC-SHA512, 페이지마다 랜덤 IV | 여러 방식, 기본 ChaCha20-Poly1305 (sqleet). SQLCipher 방식 호환 모드도 있음 | 페이지 단위 AEAD (AEGIS, AES-GCM) | 미조사 | 디스크 전체 |
| Rust 연동 | rusqlite/libsqlite3-sys feature로 바로 | libsqlite3-sys에 해당 feature 없음 (0.40.0 Cargo.toml 확인). libsql-ffi에만 `multiple-ciphers` | turso crate. 표준 SQLite로 열 수 없음 | 직접 빌드 | 할 일 없음 |
| 성숙도 | 오래 쓰였다. 2026-10에 4.20.0 | MIT, 활발함. Rust 경로는 직접 만들어야 함 | 실험 기능 | 상용 | - |
| 무결성 검사 | `PRAGMA cipher_integrity_check`가 페이지 HMAC을 검사 | 미조사 | 페이지 태그 | - | 없음 |
| 빌드 복잡도 | macOS는 추가 설치 없이 CommonCrypto 사용 | 높음 (직접 build.rs) | 낮음, 대신 엔진 교체 | 높음 | 없음 |
| 라이선스 | BSD 3조항 (Zetetic) | MIT | MIT | 유료 | - |

SQLCipher 사실 (Zetetic 문서):

- 원시 키 `x'…'`(64자리 hex = 32바이트)는 키 유도를 건너뛴다. 암호문(passphrase)은 PBKDF2-HMAC-SHA512 256,000회를 거친다. 키는 DB에 대한 첫 작업 전에 설정해야 한다.
- WAL과 롤백 저널의 페이지도 암호화된다. 임시 파일은 암호화되지 않으므로 `SQLITE_TEMP_STORE=2`를 권한다. libsqlite3-sys의 `bundled-sqlcipher`는 `-DSQLITE_TEMP_STORE=2`, `-DSQLITE_HAS_CODEC`로 빌드한다(build.rs 확인, 실행 시 `pragma_compile_options`에 `HAS_CODEC TEMP_STORE=2`가 나옴).
- 성능 지침: 연결을 반복해서 열고 닫지 말 것(싱글턴 연결), 트랜잭션으로 묶을 것, 인덱스를 둘 것. 문서는 "as little as 5-15% overhead"라고 하지만 아래 실측은 더 컸다.
- 키 변경은 `PRAGMA rekey`, 평문 DB를 암호화하거나 다른 키로 복사할 때는 `sqlcipher_export()`를 쓴다.

libsqlite3-sys build.rs에서 확인한 빌드 동작 (0.40.0 소스):

- `bundled-sqlcipher`는 OpenSSL 위치를 **`OPENSSL_DIR`(또는 `OPENSSL_LIB_DIR`+`OPENSSL_INCLUDE_DIR`) 환경 변수로만** 찾는다. 없고 Apple 대상이면 `-DSQLCIPHER_CRYPTO_CC`로 CommonCrypto를 쓰고 `Security`, `CoreFoundation` 프레임워크를 링크한다. 환경 변수가 있으면 해당 `libcrypto`를 동적 링크한다. Homebrew OpenSSL이 설치돼 있어도 환경 변수가 없으면 쓰지 않는다.
- SQLCipher 소스는 별도 crate `sqlcipher-src`에서 온다. 0.40.0은 `=420.0.0+sqlcipher-4.20.0-sqlite-3.54.1`, 0.39.1은 `419.0.6+sqlcipher-4.19.0-sqlite-3.53.4`다. 이 crate의 저장소는 Zetetic이 아니라 개인 저장소(LucaCappelletti94/sqlcipher-src)다 (4장 위험 참조).
- 같은 버전의 일반 `bundled`는 SQLite 3.54.1이다. SQLCipher 쪽이 SQLite 한 단계 뒤에 있다.

키체인 구성 (추천):

1. 첫 실행에 OS 난수로 32바이트를 만들고 키체인 일반 암호 항목으로 저장한다. 동기화하지 않는 항목으로 둔다.
2. 앱 시작 시 꺼내 `PRAGMA key = "x'<hex>'"`를 연결 직후 첫 문장으로 실행하고, 메모리의 키는 `zeroize`(1.9.1)로 지운다.
3. 연결은 프로세스당 쓰기 연결 하나(전용 스레드)로 두고 계속 연다.
4. 접근성 클래스(`kSecAttrAccessibleWhenUnlockedThisDeviceOnly` 등)와 ACL, Touch ID 결합은 R-16에서 정한다. Apple 문서 페이지가 본문을 돌려주지 않아 정확한 정의 문구는 **미확인**이다.
5. 관련 crate: `security-framework` 3.7.0, `keyring` 4.2.0 (상세 비교는 R-16).

성능 실측 (같은 SQLCipher 빌드에서 키 없음 = 평문, WAL, `synchronous=NORMAL`, 3회 반복):

| 작업 | 평문 | 암호화 (원시 키) | 배율 |
|---|---|---|---|
| 20만 행 INSERT (트랜잭션 1개) | 366–425 ms | 1,664–1,801 ms | 약 4.3배 |
| 전체 스캔 LIKE ×5 | 42–47 ms | 85–87 ms | 약 2배 |
| 인덱스 조회 ×2000 | 21–25 ms | 39–41 ms | 약 1.8배 |
| 파일 크기 | 11,764 KiB | 11,932 KiB | +1.4% |
| 열기 + 첫 쿼리 | - | 원시 키 0.42–0.51 ms, 암호문 키 92.7–93.2 ms | - |

빌드 복잡도 실측: `libsqlite3-sys`만 지우고 다시 빌드하는 데 `bundled-sqlcipher` 16.3초, `bundled` 15.5초가 걸렸다(release). 실행 파일은 2.64 MB 대 2.28 MB. 추가 설치는 없었다.

### Q3. 마이그레이션 도구와 "적용된 마이그레이션은 불변" 검사

| 도구 | 적용 기록 | 적용된 것 수정 감지 | rusqlite 0.41 호환 |
|---|---|---|---|
| `rusqlite_migration` 2.7.0 | `PRAGMA user_version` 하나 | **못 함.** 1번을 고친 뒤 `to_latest()`와 `validate()` 모두 `Ok(())` (실험) | 예 (0.41 요구) |
| `refinery` 0.10.x | `refinery_schema_history` 테이블 (version, name, applied_on, checksum) | `abort_divergent` 옵션이 켜져 있으면 감지. 체크섬은 SipHasher13(name, version, sql) | 0.10.1이 `<=0.41` |
| sqlx `migrate!` | `_sqlx_migrations`, SHA-384 | 감지 (`VersionMismatch`) | 해당 없음 (sqlx 전용) |
| 직접 구현 (추천) | `_migration_ledger(version, sha256)` | 감지 (실험: `Err("migration 1 changed after apply")`) | 의존성 없음 |

추천: `store-sqlite` 안에 마이그레이션 실행기를 직접 만든다 (100줄 안팎으로 예상).

- 마이그레이션은 `include_str!`로 넣은 번호 붙은 SQL 파일이다. 적용할 때마다 한 트랜잭션 안에서 SQL과 원장 행 `(version, sha256, applied_at)`을 함께 쓴다.
- 시작할 때 원장의 모든 행을 현재 파일의 해시와 비교한다. 다르거나, 원장에 있는 버전의 파일이 없거나, 번호가 비어 있으면 앱이 DB를 열지 않고 "버그" 종류 오류를 낸다.
- 하네스 검사(`xtask migration-check`, CI와 pre-push): `main`에 이미 있는 `migrations/*.sql`이 바뀌거나 지워지면 실패한다(`git diff --diff-filter=MDR origin/main -- migrations/`). 새 파일은 마지막 번호 + 1이어야 한다. 실행 시 원장은 이미 적용된 사용자 DB를 지키고, git 검사는 개발 중 실수를 막는다.
- 스키마 스냅샷 테스트: 빈 DB에 전부 적용한 뒤 `sqlite_schema`를 정렬해 덤프하고, 저장소에 둔 기대 파일과 비교한다. 의도하지 않은 스키마 변화가 리뷰에 드러난다.
- 시작 시 `PRAGMA foreign_key_check`와 `quick_check`도 돌린다.

### Q4. 한국어 뉴스 제목 전문 검색

FTS5 내장 토크나이저의 동작 (sqlite.org/fts5.html):

- `unicode61`은 유니코드 범주 L*, N*, Co를 토큰 문자로, 나머지를 구분자로 본다. 한국어는 띄어쓰기 단위가 토큰이 되므로 "삼성전자가"가 한 토큰이 된다.
- `trigram`은 "Substrings consisting of fewer than 3 unicode characters do not match any rows when used with a full-text query". 대신 LIKE/GLOB을 색인으로 처리할 수 있다. 3글자 미만 패턴은 전체 스캔으로 간다.
- 사용자 정의 토크나이저는 `fts5_api`(v2 `xCreateTokenizer_v2`)의 C API로 등록한다. rusqlite 0.42에는 이를 감싼 안전한 API가 없다(src 확인). 쓰려면 `unsafe` FFI가 필요하다.

선택지:

| 방식 | 구현 | 2글자 질의 | 조사 붙은 단어 | 단어 중간 일치 | 위험 |
|---|---|---|---|---|---|
| `unicode61` 그대로 | 없음 | 띄어쓴 단어만 | 실패 ("삼성전자" → 없음) | 실패 | 재현율이 낮다 |
| `unicode61` + 접두 질의 `"…"*` | 질의 변환 | 단어 앞부분만 | 앞부분이면 성공 | 실패 ("하이닉스" 없음) | 중간 일치를 놓친다 |
| `trigram` | 없음 | MATCH 실패, LIKE로 전체 스캔 | 성공 | 성공 | 2글자 질의가 흔한 한국어에 불리하다 |
| **앱 측 바이그램 + `unicode61`** | Rust 함수 하나 (안전한 코드) | 성공 | 성공 | 성공 | 1글자 질의 불가, 색인이 커진다, `snippet`/`highlight`가 원문을 못 보여 준다 |
| 형태소 분석 사용자 토크나이저 (lindera ko-dic, mecab-ko 등) | `unsafe` FFI + 사전 | 단어 단위 | 분석기가 조사를 떼면 성공 | 복합어는 사전 의존 | 새 종목명·신조어 미등록, 사전 크기, 의존성 무게 |

실측 (합성 제목 10개, 행 번호는 일치한 행):

```
질의         | unicode61    | unicode61 접두* | trigram      | trigram LIKE | 앱 바이그램
삼성전자       | -            | 1,5          | 1,5          | 1,5          | 1,5
금리         | 4,8          | 4,8          | -            | 3,4,8        | 3,4,8
반도체        | 1,10         | 1,10         | 1,10         | 1,10         | 1,10
하이닉스       | -            | -            | 2            | 2            | 2
전자         | -            | 9            | -            | 1,5,9        | 1,5,9
인하         | 3,4          | 3,4          | -            | 3,4          | 3,4
HBM        | 2            | 2            | 2            | 2            | 2
기준금리       | 3            | 3            | 3            | 3            | 3
실적 개선      | -            | 1            | 1            | 1            | 1
바이그램 예: 삼성 성전 전자 자가 3 분기 반도 도체 실적 개선 선을 발표 표했 했다
```

(제목 3 = "한국은행 기준금리 동결…", 9 = "전자담배 규제 강화 법안 발의")

해석:

- 바이그램은 trigram LIKE와 같은 부분 문자열 의미를 색인 질의로 얻었다. "전자"가 "전자담배"에도 맞는 것은 부분 문자열 검색의 성질이다. 순위는 bm25로 정한다.
- 바이그램 질의는 질의어도 같은 함수로 쪼개 구문(phrase) 질의 `"하이 이닉 닉스"`로 만든다. 인접 조건이 걸려 있어서 흩어진 바이그램끼리 잘못 맞지는 않는다.
- 형태소 분석기의 품질은 **미측정**이다. `lindera-sqlite` 2.0.0은 C ABI 로드형 확장(cdylib)이고 lindera 2.0.1에 묶여 있다(최신 lindera는 6.2.0). 앱 안에서 정적으로 등록할 수 있는지는 미확인이다. `lindera-ko-dic` 6.2.0 crate 파일은 25 KB이고, 사전 본체를 빌드 때 받는지와 그 크기도 미확인이다.

추천 스키마: 뉴스 원본 테이블(`news_item`)과 별개로 `CREATE VIRTUAL TABLE news_fts USING fts5(title_bi, tokenize='unicode61', content='', contentless_delete=1)`을 두고 rowid로 원본과 잇는다. contentless-delete는 SQLite 3.43.0부터이며 현재 번들 엔진(3.53.4)에서 쓸 수 있다. 바이그램 함수와 질의 변환은 `store-sqlite`에 두고 버전 상수를 붙인다. 함수를 바꾸면 `'delete-all'` 후 다시 색인한다. 1글자 질의는 원본 테이블 LIKE로 처리한다. 뉴스 양이 적어 전체 스캔으로도 충분하다.

### Q5. 추가만 되는 감사 로그와 변조 탐지

다섯 겹을 실험으로 확인했다.

| 겹 | 막는 것 | 실험 결과 |
|---|---|---|
| ① STRICT 테이블 + `BEFORE UPDATE`/`BEFORE DELETE` 트리거 `RAISE(ABORT)` | 실수로 하는 수정·삭제 | `Err("audit_log is append-only")` |
| ② `seq` 연속 검사 트리거 (`NEW.seq = max+1`) | 건너뛴 번호 | `Err("audit_log seq must be contiguous")` |
| ③ 연결 authorizer (rusqlite `hooks`): audit_log의 UPDATE, DELETE, DROP TABLE/TRIGGER, ALTER를 Deny | 같은 연결에서 트리거를 지우고 고치는 것 | 네 가지 모두 `not authorized`, INSERT만 통과 |
| ④ `SQLITE_DBCONFIG_DEFENSIVE` | `writable_schema`로 스키마를 직접 고치는 것 | `table sqlite_master may not be modified` |
| ⑤ 해시 체인 `hash = SHA-256(len‖seq, len‖at, len‖kind, len‖payload, len‖prev_hash)` + 외부 앵커(마지막 seq와 hash) | 위 겹을 우회한 변조, 꼬리 삭제, 체인 재작성 | 트리거를 지우고 1번을 고치면 `content hash mismatch at 1`. 전체를 지우고 다시 쓰면 `head != external anchor` |

설계상 한계 (정직하게 적는다):

- DB 키를 가진 쪽(이 앱 자신, 키체인 접근이 되는 사용자 세션의 다른 코드)은 체인을 처음부터 다시 계산할 수 있다. 이것을 잡는 수단은 **DB 밖의 앵커**뿐이다. 앵커 후보는 키체인 항목(사건마다 갱신), 일일 백업 파일, 사용자 알림에 찍는 해시 접두어다. 사건마다 키체인에 쓰는 비용과 실패 처리는 미측정이다.
- 키가 없는 쪽은 SQLCipher의 페이지 HMAC 때문에 읽지도 고치지도 못한다(Q7 실험: 1바이트 변조 → `HMAC verification failed for page 51`).
- 해시 입력은 길이를 앞에 붙인 정규형이어야 한다. JSON 페이로드는 저장 전에 정규화한 문자열을 그대로 저장하고 다시 직렬화하지 않는다.
- authorizer는 연결마다 등록해야 한다. 연결을 만드는 함수를 하나로 두고 테스트로 확인한다.
- ORD-2의 "전송 전에 먼저 기록"은 감사 로그 INSERT와 커밋이 전송보다 앞서는지 검사하는 순서 테스트로 지킨다.

### Q6. 금액 열 형식

토스 스펙(docs/toss-openapi-1.2.28.json)은 금액·수량·비율을 `format: decimal` 문자열로 주며 패턴은 `^-?\d+(\.\d+)?$`, `^\d+(\.\d+)?$`다. 소수 자릿수는 필드마다 다르다 (비율 `0.03215`, 환율 `1380.5`, 정수 금액 `41250000000`).

| 형식 | 정확성 | SQL 합산·정렬 | 스키마 변경 위험 | 판정 |
|---|---|---|---|---|
| REAL | 실패: `0.123456789012345678` → `0.12345678901234568` | 됨 (부정확) | - | 금지 |
| NUMERIC | 실패: REAL로 바뀜 (`typeof = real`) | 됨 (부정확) | - | 금지 |
| INTEGER 최소 단위 (원=1, 센트=1/100, 수량=1e-8 등 열마다 고정 배율) | 정확 | 정확 | 토스가 자릿수를 늘리면 배율 변경 마이그레이션이 필요하다. 소수점 주식 수량의 최대 자릿수는 미확인이다 (R-01) | 차선 |
| **TEXT 정규 십진수 + CHECK + 통화 열** | 정확, API 문자열과 손실 없이 왕복 | SQL `sum()`은 REAL로 바뀐다 (`0.1+0.2 = 0.30000000000000004`). 정렬은 사전순 (`10,2.5,9`) | 자릿수 변화에 영향 없음 | **추천** |
| BLOB (`rust_decimal` 16바이트) | 정확 | 불가 | 사람이 읽을 수 없음 | 비추천 |

추천 규칙:

- 열 정의: `amount TEXT NOT NULL CHECK (amount GLOB '[0-9]*' OR amount GLOB '-[0-9]*') CHECK (amount NOT GLOB '*[^0-9.-]*') CHECK (amount NOT GLOB '*.*.*')`, `currency TEXT NOT NULL CHECK (currency IN ('KRW','USD'))`, 테이블은 STRICT. 실험에서 `'1e5'`와 `'12.5.1'`이 CHECK에 걸렸다. 정규식과 완전히 같은 검사는 아니다. 최종 검증은 Rust의 `Money`/`Quantity` 파서가 하고, CHECK는 두 번째 그물이다.
- 저장 형식은 `rust_decimal::Decimal`의 정규 문자열 하나로 고정한다(지수 표기 없음, `+` 없음). 뒤쪽 0을 지울지(`normalize`)는 하나로 정해 도메인 테스트로 고정한다. 해시 체인과 동등 비교가 이 형식에 의존하기 때문이다.
- SQL에서 금액 열에 `sum`, `avg`, `ORDER BY`, 산술을 쓰지 않는다. 합산과 정렬은 Rust에서 한다(실험: `rust_decimal` 합 = `0.3`). 이 규칙은 하네스 검사(SQL 상수 정적 검사)로 강제한다.
- `rust_decimal` 1.43.0은 가수 96비트(최대 `79228162514264337593543950335`)에 소수 28자리까지다. 토스 예시의 최대값(시가총액 `388035900000000`)과 원·달러 환산 곱셈(`1384.55 × 0.123456789 = 170.93209720995`)은 범위 안에 든다. 곱셈을 이어 할 때 자릿수가 넘치면 오류가 나므로 `checked_mul`과 명시적 반올림 규칙을 R-11에서 정한다.

### Q7. 백업, 복구, 손상 탐지

실험 결과 (20만 행, 약 12 MB 암호화 DB):

| 방법 | 결과 |
|---|---|
| 온라인 백업 API, 대상도 같은 키 | 53–61 ms. 대상도 암호화됨. 복원본 `quick_check = ok`, `cipher_integrity_check` 오류 0행 |
| 온라인 백업 API, 대상이 **다른 키** | 성공. 결과는 **대상 키로만** 열리고 원본 키로는 `file is not a database`. 백업 전용 키를 쓸 수 있다 |
| 온라인 백업 API, 대상에 키 없음 | `backup is not supported with encrypted databases` (평문 백업이 실수로 생기지 않는다) |
| `VACUUM INTO` | 73–103 ms. 결과도 원본 키로 암호화돼 열린다 |
| `ATTACH … KEY` + `sqlcipher_export()` | 64–88 ms. 새 키로 열린다 |
| 백업 사본의 1바이트 변조 | `quick_check`와 `integrity_check`는 `btreeInitPage() returns error code 11`을 대량으로 내고, `cipher_integrity_check`는 `HMAC verification failed for page 51`로 페이지를 정확히 짚었다. 전체 읽기는 `database disk image is malformed` |

SQLite 문서 사실:

- 백업 API는 같은 연결에서 원본을 쓰면 대상도 함께 갱신되고 계속 진행된다. 다른 연결이나 프로세스가 쓰면 다시 시작하며, 너무 자주 다시 시작하면 끝나지 않을 수 있다.
- `VACUUM INTO`는 결과가 최소 크기이고 지운 내용이 남지 않는다. 백업 API는 CPU를 덜 쓰고 나눠서 실행할 수 있다. 원본이 `synchronous=NORMAL` 이상이면 결과 파일을 fsync한다. 대상 파일은 없거나 비어 있어야 한다.

추천 (REL-6 구체화):

1. 하루 한 번 앱의 쓰기 연결로 백업 API를 실행한다. 쓰기 연결과 같은 연결이라 다시 시작하는 일이 없다. 결과는 `backups/YYYY-MM-DD.db.tmp`에 쓴다.
2. 끝나면 백업 파일을 따로 열어 `cipher_integrity_check`(0행)와 `quick_check`(`ok`)를 돌리고, 감사 로그 해시 체인을 검증한 뒤 원자적 이름 변경으로 확정한다. 7일이 지난 것은 지운다.
3. 앱 시작 시 `quick_check`를 돌린다. 전체 `integrity_check`와 `cipher_integrity_check`는 백업 때와 주 1회로 둔다. 데이터 크기 기준 비용은 미측정이다.
4. 복구 화면: 손상이 감지되면 가장 최근의 검증된 백업을 고르게 하고, 현재 파일은 지우지 않고 옆으로 옮긴다.
5. **키를 잃으면 모든 백업도 못 연다.** 키체인 항목을 "이 기기에서만"으로 두면 Mac을 바꾸거나 키체인이 초기화될 때 데이터를 잃는다. 사용자가 암호문으로 보호한 내보내기 파일(`sqlcipher_export`, KDF 기본값)을 만들 수 있게 할지는 R-16과 함께 정한다.

### Q8. 스냅샷 테이블 크기 관리

실험: 30종목을 장중 10분마다(하루 39회), 250영업일(1년치) 저장했다. `snapshot` 9,750행과 `snapshot_holding` 292,500행이다. `snapshot_holding`은 STRICT, `WITHOUT ROWID`, PK `(snapshot_id, symbol)`, 금액은 TEXT다.

| 단계 | 파일 크기 | page_count | freelist |
|---|---|---|---|
| 1년치 저장 | 15,464 KiB | 3,866 | 0 |
| 30일 이전 장중 스냅샷을 "하루 마지막 1개"로 줄인 뒤 (`ON DELETE CASCADE`, 208–231 ms) | 15,464 KiB | 3,866 | 3,273 |
| `PRAGMA incremental_vacuum` 실행 후 | 2,356 KiB | 589 | 0 |

배운 점과 추천:

- 행을 지워도 파일은 줄지 않는다. 빈 페이지가 freelist로 간다. `auto_vacuum=INCREMENTAL`은 **테이블을 만들기 전**(또는 바로 뒤 VACUUM)에 설정해야 한다. `PRAGMA incremental_vacuum`은 결과 행을 끝까지 읽어야 전부 반환된다. 처음에 `execute_batch`로 한 번만 실행했을 때는 1페이지만 줄었다(실험에서 겪은 함정).
- 보존 정책(추천): 장중 스냅샷은 30일 보관하고 그 뒤에는 일별 종가 스냅샷만 영구 보관한다. 거래내역 원본, 감사 로그, 제안은 줄이지 않는다. 이 조건이면 1년 장중 원본이 15 MB이고, 줄인 뒤 남는 양은 연 2~3 MB 수준이다.
- 다른 선택지 (미실험): 직전 스냅샷과 같은 종목·수량이면 저장하지 않는 "변경 시만 저장" 방식과, 시세 평가액을 저장하지 않고 수량만 저장한 뒤 일봉으로 다시 계산하는 방식이 있다. 후자는 "코드가 계산한 값을 재현"하는 데 유리하지만 과거 평가 시점의 시세를 보존해야 한다.
- 정리 작업은 스케줄러의 하루 1회 작업으로 두고 백업 직전에 실행한다.

### 실험

명령:

```
cargo +1.95.0 build --release   # rusqlite =0.41.1 features=["bundled-sqlcipher","backup","hooks"], rusqlite_migration =2.7.0, rust_decimal =1.43.0
./target/release/r06-exp out     # 3회 반복
./target/release/auth            # authorizer 실험
DATABASE_URL=sqlite://…/schema.db cargo +1.95.0 build --release --bin macro   # sqlx 0.9 매크로 실험
```

연결 열기 (핵심):

```rust
let c = Connection::open(p)?;
c.execute_batch(&format!("PRAGMA key = \"x'{hexkey}'\";"))?; // 첫 문장. 원시 키라 KDF 없음
c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;")?;
```

버전 확인 출력:

```
sqlite_version   = Text("3.53.4")
cipher_version   = Text("4.19.0 community")
cipher_provider  = Text("commoncrypto")
cipher_provider_version = Text("61901.120.67")
fts5 compiled    = Integer(1)
TEMP_STORE opt   = Text("HAS_CODEC TEMP_STORE=2")
file header[0..16] = (난수 바이트, "SQLite format 3" 아님)
키 없이 읽기      = ERR file is not a database
틀린 키로 열기   = ERR file is not a database
```

`otool -L` 결과: `Security.framework`, `CoreFoundation.framework`, `libSystem`, `libiconv`뿐이다. Homebrew OpenSSL에 의존하지 않는다.

감사 로그 트리거 (핵심):

```sql
CREATE TRIGGER audit_no_update BEFORE UPDATE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;
CREATE TRIGGER audit_no_delete BEFORE DELETE ON audit_log BEGIN SELECT RAISE(ABORT, 'audit_log is append-only'); END;
CREATE TRIGGER audit_seq BEFORE INSERT ON audit_log
  WHEN NEW.seq IS NOT (SELECT coalesce(max(seq),0)+1 FROM audit_log)
  BEGIN SELECT RAISE(ABORT, 'audit_log seq must be contiguous'); END;
```

authorizer 출력:

```
DEFENSIVE 켜기 = Ok(true)
DROP TRIGGER audit_no_update                  => Err("not authorized")
UPDATE audit_log SET payload='b'              => Err("not authorized")
DELETE FROM audit_log                         => Err("not authorized")
DROP TABLE audit_log                          => Err("not authorized")
INSERT INTO audit_log VALUES(2,'b')           => Ok(())
PRAGMA writable_schema=ON; UPDATE sqlite_schema … => Err("table sqlite_master may not be modified")
```

sqlx 실험 출력:

```
SELECT sqlite_version() = 3.50.4
PRAGMA cipher_version = 4.10.0 community
PRAGMA cipher_provider = commoncrypto
키 없이 = Err("error returned from database: (code: 26) file is not a database")
error: error returned from database: (code: 1) no such column: nope        ← query! 컴파일 오류
error[E0308]: expected `String`, found `Option<String>`                     ← LEFT JOIN 열 NULL 추론
```

## 3. 선택지 비교표와 추천

| 결정 | 추천 | 차선 | 이유 |
|---|---|---|---|
| 드라이버 | **rusqlite 0.41.1** (`=` 고정, features `bundled-sqlcipher`, `backup`, `hooks`) | sqlx 0.9 + libsqlite3-sys 0.37 `bundled-sqlcipher` | 최신 SQLCipher(4.19)와 SQLite(3.53.4), authorizer와 백업 API를 바로 쓸 수 있다. 동기 API가 단일 쓰기 연결 구조와 맞는다. 컴파일 시점 검사가 없는 단점은 STRICT, prepare 테스트, 왕복 테스트로 메운다. sqlx는 매크로가 강점이지만 엔진이 오래된 버전에 묶이고 MSRV가 1.94다 |
| 비동기 연결 | 전용 DB 스레드 하나 + 채널 (직접 구현) | `tokio-rusqlite` | tokio-rusqlite 0.8.0은 rusqlite 0.40에 묶여 있다. 단일 쓰기 연결은 SQLCipher 성능 지침과도 맞다 |
| 암호화 | **SQLCipher (CommonCrypto), 키체인의 32바이트 원시 키** | SQLite3MultipleCiphers (직접 빌드) | 추가 설치가 없고, 페이지 HMAC으로 변조와 손상을 탐지하며, 백업 API가 암호화 상태를 유지한다 |
| 마이그레이션 | **직접 구현 + SHA-256 원장 + git 불변 검사 + 스키마 스냅샷 테스트** | refinery (`abort_divergent`) | rusqlite_migration은 수정을 못 잡는다(실험). refinery는 rusqlite 버전 범위 때문에 업그레이드가 막힌다 |
| 금액 열 | **STRICT TEXT 정규 십진수 + CHECK + 통화 열, 계산은 Rust** | 열마다 고정 배율 INTEGER | 토스 API 문자열과 손실 없이 왕복하고 자릿수 변화에 강하다. ACC-2와 같다 |
| 한국어 검색 | **앱 측 바이그램 + `unicode61` contentless-delete FTS5** | trigram + 짧은 질의는 LIKE | 2글자 질의, 조사, 중간 일치를 모두 잡고 unsafe 코드가 없다 |
| 감사 로그 | 트리거 + seq 트리거 + authorizer + DEFENSIVE + 해시 체인 + 외부 앵커 | 트리거 + 해시 체인만 | 실험으로 모든 겹을 확인했다 |
| 백업 | 쓰기 연결의 백업 API → 검증 → 원자적 이름 변경, 7일 | `VACUUM INTO` | CPU를 덜 쓰고 같은 연결이라 다시 시작이 없다 |

추천이 틀렸다고 판명될 조건:

- rusqlite `bundled-sqlcipher`가 쓰는 `sqlcipher-src` crate가 Zetetic 원본과 다르거나 갱신이 끊긴 것으로 드러나면, SQLCipher를 직접 빌드(Zetetic 소스와 build.rs)하거나 시스템 SQLCipher(`sqlcipher` feature)로 바꾼다.
- 저장소 코드에서 SQL 오타·타입 오류가 테스트를 통과하고 실제로 반복 발견되면, sqlx로 바꿀 가치가 생긴다(엔진 버전이 낮아지는 대가).
- 단일 쓰기 연결이 UI 응답을 막을 만큼 느려지면(예: 백업이나 재색인 중 수백 ms 이상 대기), 읽기 전용 연결 풀을 추가한다.
- 토스 API가 금액을 지수 표기나 `+` 부호로 주는 사례가 나오면 CHECK 규칙과 파서를 고친다. 스펙 패턴상으로는 나오지 않는다.
- 실제 뉴스 수만 건에서 바이그램 검색의 오탐(부분 문자열 일치) 때문에 순위 품질이 나쁘면 형태소 분석기 토크나이저를 다시 검토한다.
- 사건마다 키체인 앵커를 갱신하는 비용이나 실패가 문제가 되면 앵커를 백업 시점과 알림으로만 둔다.

## 4. 위험과 미확인 항목

위험:

- **공급망**: SQLCipher 소스가 Zetetic 공식 배포가 아니라 개인이 관리하는 `sqlcipher-src` crate(LucaCappelletti94/sqlcipher-src)에서 온다. libsqlite3-sys가 `=420.0.0`처럼 정확한 버전으로 고정하지만, 내용이 공식 소스와 같은지는 **미확인**이다. 완화책: `cargo vet`/`cargo deny`로 고정하고, 업그레이드 때 amalgamation 해시를 Zetetic 태그 빌드와 비교하는 작업을 하네스에 추가한다.
- **버전 잠금**: libsqlite3-sys `links` 제약 때문에 rusqlite를 올리면 마이그레이션·비동기 crate가 따라오지 못한다. 직접 구현을 추천하는 이유이기도 하다. rusqlite 0.42(오늘 배포)로 올리는 것은 주변 crate가 따라온 뒤로 미룬다.
- **MSRV**: rusqlite 0.41 이상은 Rust 1.89가 필요하다. 이 PC의 기본 툴체인은 1.88.0이다. `rust-toolchain.toml` 고정이 필요하다(R-09).
- **성능 수치**: 문서의 "5-15%"와 달리 쓰기 약 4배, 읽기 약 2배였다. 합성 벤치 한 종류이고 `cipher_page_size`, `cache_size`를 조정하지 않은 값이다. 실제 부하에서 다시 잰다.
- **키 분실 = 전체 데이터와 백업 분실**: 복구 경로를 R-16에서 정해야 한다.
- **OPENSSL_DIR 오염**: 빌드 환경에 `OPENSSL_DIR`가 있으면 CommonCrypto 대신 그 libcrypto를 동적 링크한다. CI와 xtask에서 `cipher_provider = commoncrypto`이고 `otool -L`에 Security 프레임워크만 있는지 검사해야 한다.
- 해시 체인은 키를 가진 코드의 재작성을 외부 앵커 없이는 잡지 못한다.

미확인:

- Apple `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`의 공식 정의 문구 (페이지가 본문을 돌려주지 않음). 접근성 클래스 선택은 R-16으로 넘긴다.
- diesel 2.3에서 SQLCipher 동작 (미실험), diesel과 turso의 MSRV.
- SQLite3MultipleCiphers의 현재 버전과 기반 SQLite 버전 (문서에 없음).
- 형태소 분석 토크나이저(lindera ko-dic, mecab-ko)의 한국어 뉴스 제목 품질, 사전 크기, 앱 내 정적 등록 가능 여부.
- 바이그램 색인과 trigram 색인의 실제 크기 비교 (뉴스 수만 건 기준, 미측정).
- 토스 소수점 주식 수량의 최대 소수 자릿수 (R-01). INTEGER 배율 방식을 택하려면 필요하다.
- 대형 DB(수백 MB)에서 `integrity_check`와 `cipher_integrity_check`에 걸리는 시간.
- 사건마다 키체인 앵커를 쓸 때의 비용과 실패 동작.
- Turso 매뉴얼의 "No triggers" 제한이 최신 엔진에도 해당하는지 (매뉴얼 문구만 확인).

## 5. 이 결과로 바뀌어야 할 설계 문서 항목

| 문서 | 항목 | 바꿀 내용 |
|---|---|---|
| architecture.md 8장 결정 기록 | "SQLite 드라이버와 암호화 방식 — R-06" | ADR로 확정: rusqlite 0.41.1 (`=` 고정, `bundled-sqlcipher`/`backup`/`hooks`), SQLCipher 4.x + CommonCrypto, 키체인의 32바이트 원시 키 |
| architecture.md 3.2 의존 규칙 | `adapters/store-sqlite` | rusqlite 의존을 이 crate에만 허용하는 규칙 추가 (`xtask arch-check`). `secrets-keychain`과 직접 의존하지 않고, 키는 조립 루트가 `SecretStore`에서 꺼내 넘긴다 |
| architecture.md 6장 데이터 | 표 전체 | 추가할 내용: 모든 테이블 STRICT. 금액·수량·비율은 TEXT 정규 십진수 + CHECK + 통화 열. SQL에서 금액 산술·합산·정렬 금지. 뉴스·공시 검색은 "앱 측 바이그램 FTS5(contentless-delete)". 감사 로그는 "트리거 + seq 트리거 + authorizer + DEFENSIVE + 해시 체인 + 외부 앵커". 스냅샷 보존 정책(장중 30일 → 일별 1개, `auto_vacuum=INCREMENTAL`) |
| architecture.md 6장 마이그레이션 문장 | "이미 적용된 마이그레이션은 고치지 않는다" | 검사 방법 명시: SHA-256 원장(실행 시) + `xtask migration-check`(git) + 스키마 스냅샷 테스트 |
| design.md SEC-6 | "SQLite는 암호화하고 키는 키체인에 둔다" | "SQLCipher, 256비트 난수 원시 키(KDF 없음), `TEMP_STORE=2`, WAL 포함 암호화. 키는 키체인에만, 메모리에서는 사용 후 지움"으로 구체화 |
| design.md REL-6 | 하루 한 번 백업, 7일 보관 | "쓰기 연결의 온라인 백업 API로 암호화 백업 → `cipher_integrity_check`와 `quick_check`, 해시 체인 검증 → 원자적 확정"과 "키 분실 시 복구 불가, 복구 경로는 R-16"을 추가 |
| design.md ACC-2 | 금액은 문자열 십진수 | DB 저장 형식도 같은 정규 문자열이라는 점과 SQL 산술 금지를 추가 |
| design.md ORD-3 | 지울 수 없는 감사 로그 | "변조 탐지 범위: 키 없는 쪽은 HMAC, 키 있는 쪽은 외부 앵커로 탐지"를 명시 |
| harness.md | Rust 규칙 | (1) store-sqlite의 SQL 상수 전부를 마이그레이션된 DB에 `prepare`하는 테스트 필수. (2) SQL 상수에서 금액 열에 `sum/avg/ORDER BY/산술` 금지 정적 검사. (3) 빌드 산출물의 `cipher_provider = commoncrypto`와 링크 대상 검사. (4) `rust-toolchain.toml` ≥ 1.89 |
| tasks.md S-01 | 완료 기준 "적용된 마이그레이션을 고치면 실패" | "원장 해시 불일치·누락·번호 공백 시 열기 실패, git 불변 검사, 스키마 스냅샷 테스트, 키 없이·틀린 키로 열면 실패, `cipher_provider` 검사"로 확장 |
| tasks.md S-05 | 한국어 제목 검색 테스트 | 2글자 질의("금리"), 조사 붙은 단어("삼성전자가" ← "삼성전자"), 중간 일치("하이닉스" ← "SK하이닉스"), 영문 혼합("HBM"), 1글자 질의 LIKE 대체를 테스트 사례로 명시 |
| tasks.md S-08 | 감사 로그 | authorizer와 DEFENSIVE 겹, 외부 앵커와 대조하는 재작성 탐지 테스트를 완료 기준에 추가 |
| tasks.md S-11 | 백업 | 백업 파일 검증(무결성과 체인)과 손상 사본으로 복구하는 테스트, 7일 정리 |
| tasks.md (신규) | 스냅샷 보존 작업 | 장중 스냅샷 다운샘플과 `incremental_vacuum`을 하는 예약 작업 추가 (S-02 이후, 스케줄러 의존) |
| research.md R-11·R-16 | 연결 | R-11: `rust_decimal` 범위(28자리)와 곱셈 반올림 규칙. R-16: DB 키 항목의 접근성 클래스, 키 분실 복구(암호문 내보내기), 앵커 저장 위치 |

## 6. 출처 목록

모두 2026-11-11에 확인했다.

1차 출처 (공식 문서, 소스, 레지스트리):

- SQLCipher API (`PRAGMA key` 원시 키, `kdf_iter`, `cipher_integrity_check`, `cipher_provider`, `sqlcipher_export`, `rekey`): https://www.zetetic.net/sqlcipher/sqlcipher-api/
- SQLCipher 설계 (AES-256-CBC, HMAC-SHA512, PBKDF2 256,000회, WAL/저널 암호화, 임시 파일 주의): https://www.zetetic.net/sqlcipher/design/
- SQLCipher 성능 지침 ("5-15% overhead", 연결 재사용): https://www.zetetic.net/sqlcipher/performance/
- SQLCipher 변경 기록 (4.16.0–4.20.0): https://github.com/sqlcipher/sqlcipher/blob/master/CHANGELOG.md
- SQLCipher 라이선스 (BSD 3조항): https://github.com/sqlcipher/sqlcipher/blob/master/LICENSE.md
- SQLite FTS5 (trigram 3글자 제한, unicode61 범주, 사용자 토크나이저 v2, contentless-delete 3.43.0): https://www.sqlite.org/fts5.html
- SQLite 온라인 백업 API: https://www.sqlite.org/backup.html
- SQLite VACUUM / VACUUM INTO: https://www.sqlite.org/lang_vacuum.html
- SQLite STRICT 테이블: https://www.sqlite.org/stricttables.html
- SQLite 타입 친화성 (NUMERIC → REAL 변환, 15자리): https://www.sqlite.org/datatype3.html
- SQLite PRAGMA (`auto_vacuum`, `incremental_vacuum`, `quick_check`, `integrity_check`): https://www.sqlite.org/pragma.html
- SQLite authorizer: https://www.sqlite.org/c3ref/set_authorizer.html
- SQLite `SQLITE_DBCONFIG_DEFENSIVE`: https://www.sqlite.org/c3ref/c_dbconfig_defensive.html
- crates.io API와 crate 소스(내려받아 Cargo.toml과 소스를 직접 읽음):
  - rusqlite 0.42.0 / 0.41.1: https://crates.io/crates/rusqlite (feature 목록, MSRV 1.89.0, `src/hooks/mod.rs` authorizer, `src/config.rs` DEFENSIVE)
  - libsqlite3-sys 0.40.0 / 0.39.1: https://crates.io/crates/libsqlite3-sys (`build.rs`의 OpenSSL/CommonCrypto 선택, 컴파일 플래그 FTS5·TEMP_STORE=2, `links = "sqlite3"`, 번들 SQLite 3.54.1)
  - sqlcipher-src 420.0.0 / 419.0.6: https://crates.io/crates/sqlcipher-src , 저장소 https://github.com/LucaCappelletti94/sqlcipher-src
  - sqlx 0.9.0, sqlx-sqlite 0.9.0, sqlx-core 0.9.0: https://crates.io/crates/sqlx (README 매크로·DATABASE_URL, `sqlx-sqlite/src/options/mod.rs` key pragma 순서, `src/types/mod.rs` decimal 미지원 이유, `sqlx-core/src/migrate/migrator.rs` 체크섬)
  - rusqlite_migration 2.7.0: https://crates.io/crates/rusqlite_migration (`src/lib.rs` user_version)
  - refinery-core 0.10.0 / 0.10.1: https://crates.io/crates/refinery-core (`src/traits/mod.rs` abort_divergent, `src/runner.rs` SipHasher13, rusqlite 범위)
  - diesel 2.3.14: https://crates.io/crates/diesel (libsqlite3-sys 범위)
  - turso 0.8.3: https://crates.io/crates/turso
  - tokio-rusqlite 0.8.0: https://crates.io/crates/tokio-rusqlite
  - tauri-plugin-sql 2.6.0: https://crates.io/crates/tauri-plugin-sql (sqlx 0.8 의존)
  - rust_decimal 1.43.0: https://crates.io/crates/rust_decimal
  - lindera-sqlite 2.0.0: https://crates.io/crates/lindera-sqlite , lindera-ko-dic 6.2.0: https://crates.io/crates/lindera-ko-dic , fts5-cjk 0.1.1: https://crates.io/crates/fts5-cjk
  - security-framework 3.7.0, keyring 4.2.0, zeroize 1.9.1: https://crates.io/crates/security-framework , https://crates.io/crates/keyring , https://crates.io/crates/zeroize
- SQLite3MultipleCiphers 문서: https://utelle.github.io/SQLite3MultipleCiphers/
- Turso 암호화 문서: https://docs.turso.tech/tursodb/encryption
- Turso 매뉴얼 ("not production ready", "No triggers"): https://github.com/tursodatabase/turso/blob/main/docs/manual.md
- Apple `kSecAttrAccessibleWhenUnlockedThisDeviceOnly` (본문 미확인): https://developer.apple.com/documentation/security/ksecattraccessiblewhenunlockedthisdeviceonly
- 저장소 자료: `docs/toss-openapi-1.2.28.json` (금액 문자열 패턴 `^-?\d+(\.\d+)?$`와 필드 예시)

보조 출처:

- Turso 블로그, 로컬 암호화 소개 (AEGIS/AES-GCM, 성능 주장): https://turso.tech/blog/introducing-fast-native-encryption-in-turso-database
