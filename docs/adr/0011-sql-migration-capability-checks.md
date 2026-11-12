# ADR 0011: SQL·마이그레이션·capability 검사 (`xtask sql-check`, `migration-check`, `capability-check`)

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-31, H-32, H-33
- 반영한 문서: 없음 (harness.md 3.5, 6.8, architecture.md 3.6, 6.3, design.md SEC-10, ACC-2, ADR 0004를 따르고 문서가 정하지 않은 곳만 여기 적는다)

## 맥락

harness.md 3.5는 세 검사를 이름만 정했다. architecture.md 6.3은 "금액·수량·비율 열에 합산, 평균, 산술, `ORDER BY` 금지", "모든 SQL은 `store-sqlite` 안의 상수", "git에서 적용된 마이그레이션 파일 수정 금지, 번호 공백이면 실패"를 정했고, 3.6은 capability `main` 하나, 생성된 명령 목록과 `allow-*`의 일치, 금지 권한 목록을 정했다. 금액 열을 알아보는 방법, "적용됨"의 기준 커밋, 마이그레이션 폴더와 이름 형식, 명령 목록 파일과 명령 허용 항목 파일의 위치·형식은 문서에 없다. `store-sqlite`와 명령은 아직 없다.

## 실측

환경: macOS 26 (Apple Silicon), Rust 1.99.1, `tauri` 2.12.5, `tauri-build` 2.7.4, `syn` 2.0.119, just.

| 항목 | 명령 | 결과 |
|---|---|---|
| 권한 묶음 하나를 참조하는 capability | `capabilities/main.json`이 `"commands"`를, `permissions/commands.toml`이 빈 `[[set]]`을 두고 `cargo build --locked -p src-tauri` | 빌드 성공. 기본 `tauri_build::build()`가 `permissions/`를 앱 권한으로 읽는다 |
| 없는 권한 참조 | `main.json`의 `"commands"`를 `"nonexistent"`로 | 빌드 실패 "Permission nonexistent not found, expected one of commands, core:default, …" |
| 명령 권한 이름 | `tauri-utils-2.10.2/src/acl/build.rs` `autogenerate_command_permissions` | `allow-<명령의 _를 -로>` |
| 마이그레이션 잠금의 해시 | `shasum -a 256`과 xtask의 SHA-256 비교, FIPS 180-2 예제 세 개(빈 문자열, `abc`, 448비트) | 같다 |
| 하네스 자체 테스트 | `cargo nextest run -p xtask sql_check migration_check capability_check` | 25개 통과. SQL 문장 예제 44건, 소스 예제 24건, 인식 예제 55건, 마이그레이션 사례 22건, capability 사례 35건 |
| 원소 하나 빼기 | `NON_MONEY_RESULT_FUNCTIONS`에서 `length`, `ORDER_BY_END`에서 `limit`, `release`의 후속어, 금액 조각 `price`, 금지 권한 `fs:*`를 하나씩 빼고 같은 명령 | 매번 해당 사례 테스트가 실패했다 |
| `TAURI_CONFIG` | 실제 `justfile`을 `import`한 justfile과 그렇지 않은 justfile에서 `TAURI_CONFIG`를 출력하는 레시피 | 앞쪽은 `unset`, 뒤쪽은 넣은 값. `unexport TAURI_CONFIG`를 지우면 테스트가 실패한다 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 금액 열은 이름으로 정한다. 열 이름을 밑줄·영숫자 아닌 글자·소문자→대문자 경계로 나눈 조각 중 하나라도 `harness/sql-check/money-columns.toml`의 `segments`에 있으면 금액 열이다. 목록은 금액·수량·비율·확률·임계값 이름 31개다(`amount`, `price`, `quantity`, `qty`, `shares`, `ratio`, `rate`, `weight`, `percent`, `pct`, `probability`, `threshold`, `value`, `cost`, `fee`, `tax`, `pnl` 등) | 문서에 목록이 없다. 스키마는 아직 없으므로 이름 규칙만 미리 강제할 수 있다. 조각 단위라서 `fx_rate`, `"depositKrw"`, `return_1y`가 걸린다 | 스키마의 CHECK에서 금액 열 찾기(마이그레이션이 없다), 부분 문자열 비교(`generated`의 `rate`, `values`의 `value` 오탐) | architecture.md 6.3 |
| 금액 열이 들어간 `sum`, `avg`, `total`, `min`, `max`, `abs`, `round`, `cast` 호출, `+ - * / %`의 피연산자, `< > <= >=`와 `BETWEEN`의 피연산자, `ORDER BY` 절을 실패시킨다. `ORDER BY 2`처럼 위치 번호로 정렬하면 열과 상관없이 실패한다. `AS`로 금액 열에 붙인 별칭도 금액 열이다. `length`, `typeof`, `count`의 결과는 금액이 아니다 | 문서의 "합산·평균·산술·정렬"에 더해, TEXT 금액의 크기 비교와 `min`/`max`는 사전순 정렬과 같은 오류다. 위치 번호와 `CAST`는 우회로다 | 문서 네 가지만 | ACC-2, 리뷰 |
| SQL로 보는 문자열: 앞쪽의 공백, 주석(`/* … */`, `-- …` 줄), 여는 괄호를 걷어 낸 뒤 첫 낱말이 문장 시작어이고 정해진 다음 낱말(`INSERT INTO`, `CREATE TABLE`, `DELETE FROM` 등)이나 조각(`UPDATE … SET`, `WITH … AS (`)이 있는 것. `store-sqlite` 밖의 Rust 문자열과 `.sql` 파일은 `sql:outside-store`, `store-sqlite` 안에서 `const 이름: 타입 = "…";`의 초기값이 아닌 SQL 문자열은 `sql:not-constant`다 | "모든 SQL은 `store-sqlite` 안의 상수"를 강제한다. `create directory`, `commit` 같은 일반 문자열은 통과해야 한다 | 문장 시작어 하나만(xtask 자신의 `"create directory"`, git `"commit"`이 걸렸다) | architecture.md 6.3, 실측 |
| 문자열 리터럴의 값은 `syn::LitStr`·`LitByteStr`·`LitCStr`의 `value()`로 푼다. `\x70`, `\u{70}`, 줄 끝 `\`, 원시·바이트·C 문자열이 Rust와 같은 값으로 판정된다 | 손으로 쓴 이스케이프 풀이가 `\x`, `\u{…}`를 남겨 `ORDER BY \x70rice`, `"\x53ELECT …"`가 통과했다(리뷰 재현) | 직접 풀이 | 리뷰 |
| `store-sqlite` 안의 모든 문자열 상수와 리터럴, `.sql` 파일(마이그레이션 포함)에 금액 규칙을 적용한다. SQL처럼 보이지 않는 조각 상수(`"WHERE price > ?1"`)도 검사한다 | 조각 상수를 이어 붙이면 문장 시작어 판정을 피할 수 있다 | SQL로 보이는 상수만 | - |
| `store-sqlite` 안에서는 `format!`, `format_args!`, `concat!`, `write!`, `writeln!`, `include_str!`, `include_bytes!`, `stringify!` 호출을 실패시킨다(`sql:assembly:*`) | SQL 문자열 조립의 대표 경로다. `stringify!(SELECT sum(price) …)`는 문자열 리터럴 없이 SQL을 만든다(리뷰 재현). 오류 표시는 `f.write_str`로 쓸 수 있다 | 조립 허용 후 결과 검사(정적으로 알 수 없다) | architecture.md 6.3, 리뷰 |
| 마이그레이션은 `crates/adapters/store-sqlite/migrations/NNNN_소문자_이름.sql`(네 자리 번호, 1부터 빈틈없이)이다 | architecture.md 3.1이 `store-sqlite`를 저장소 어댑터로 정했다 | `migrations/`를 루트에 | architecture.md 3.1, 6.3 |
| "적용됨"은 잠금 파일 `crates/adapters/store-sqlite/migrations.lock`에 오른 것이다. 형식은 TOML 최상위 키 `migration` 하나의 배열이고, 항목마다 정확히 `number`(정수), `file`(그 번호의 파일 이름), `sha256`(소문자 16진 64자)을 갖는다. 빈 잠금은 `migration = []`이다. 잠금에 있는 파일이 없거나(이름 변경 포함) 해시가 다르면 `migration:removed`·`migration:modified`, 잠금이 없거나 형식이 틀리면 `migration:lock-format`, 번호·파일이 겹치면 `migration:lock-duplicate`, 1부터 오름차순으로 빈틈없이 이어지지 않으면 `migration:lock-order`다. 잠금에 없는 파일은 고칠 수 있고, 번호 규칙(1부터 빈틈없음, 중복 없음) 때문에 잠금의 마지막 번호 뒤에만 올 수 있다. 빈 파일은 `migration:empty`다 | git 이력 비교는 이 저장소의 흐름과 맞지 않았다: 작업 브랜치에서 추가→수정한 뒤 병합하면 영구 실패, 병합 커밋에 숨긴 수정은 통과, 기준이 에이전트가 만들거나 옮길 수 있는 브랜치 이름이었다(리뷰). 잠금은 git도 브랜치 이름도 환경 변수도 쓰지 않는다 | git `merge-base`와 이력 비교(첫 구현), 마이그레이션 실행기의 원장만 믿기(DB가 생긴 뒤에야 드러난다) | architecture.md 6.3, 리뷰 |
| 잠금에 올리는 명령은 만들지 않는다. 마이그레이션을 확정할 때 사람이 해시를 계산해(`shasum -a 256`) 잠금에 항목을 더하고 승인한다. 누가 언제 올리는가(첫 배포 전, 앱이 그 마이그레이션을 실행하기 전)는 저장소 작업 S 묶음(S-15)에서 정한다 | 에이전트가 해시를 다시 쓸 수 있는 명령이 있으면 잠금이 의미가 없다 | `xtask migration-lock` 명령 | - |
| 명령 목록 파일은 `apps/desktop/src-tauri/ipc/commands.txt`다. 한 줄에 명령 하나(`^[a-z][a-z0-9_]*$`), 오름차순, 중복 없음, 파일이 비어 있지 않으면 줄바꿈으로 끝난다. 지금은 빈 파일이다 | `build.rs`가 의존성 없이 읽을 수 있다. 앱 권한 폴더(`permissions/`)의 파일은 Tauri가 모두 권한 파일로 읽으므로 거기 둘 수 없다 | JSON(빌드 스크립트에 `serde_json` 필요) | architecture.md 3.6, 5장 |
| capability 명령 허용 항목은 생성 파일 `apps/desktop/src-tauri/permissions/commands.toml`의 `[[set]] identifier = "commands"` 하나이고, `permissions`는 명령 목록과 같은 순서의 `allow-<명령>`이다. 손으로 쓴 보호 파일 `capabilities/main.json`(창 `main`, 권한 `["commands"]`)이 이 묶음을 참조하고, `tauri.conf.json`은 `capabilities: ["main"]`이다 | capability를 `main` 하나로 두면서 생성되는 부분과 손으로 쓴 부분을 다른 파일로 나눌 수 있는 Tauri의 수단은 권한 묶음뿐이다. 명령이 없는 지금은 묶음이 비어 있어 WebView 권한이 없다 | 생성 capability 파일을 따로(`main`이 둘이 된다), `main.json` 안에 생성 항목(보호 파일을 생성기가 고친다) | architecture.md 3.6, R-05 Q1 |
| `capability-check`가 실패시키는 것: `app.security.capabilities`가 없거나 빈 목록(`not-explicit`), `["main"]`이 아님(인라인 객체 포함, `list-not-main`), `capabilities/`에 `main.json` 밖의 파일, `main.json`의 `identifier`·`windows`가 `main`이 아니거나 `webviews`·`remote`가 있음, `commands` 묶음과 허용 표에 없는 권한(`not-allowed`), 손으로 쓴 앱 명령 권한(`:`이 없는 `commands` 밖의 항목), `commands` 미참조, 묶음 파일의 모양·불일치, 명령 목록 형식, `permissions/`의 다른 파일(`autogenerated/`에는 목록의 `<명령>.toml`만), `build.rs`의 명령 이름 문자열이나 `commands(&[…])` 배열, 명령이 있는데 `build.rs`가 목록 경로·`AppManifest`·`commands`를 쓰지 않음, `tauri.*.conf.json`·`Tauri.toml` 같은 추가 설정 파일 | Tauri는 플랫폼별 설정 파일을 합치므로 그것으로 capability 목록을 바꿀 수 있다. 앱 권한 파일 하나로 아무 명령이나 허용할 수 있다 | - | SEC-10, ADR 0004 |
| capability 권한은 허용 목록이다. `main.json`은 생성된 `commands` 묶음과 보호된 표 `harness/capability-check/allowed-permissions.toml`(`reason`, `permissions`)에 적은 권한만 가질 수 있다. 지금 표는 비어 있다. 금지 권한 목록은 그 표에 금지 권한이나 앱 명령 권한을 넣지 못하게 하는 2차 검사로 쓴다 | 금지 목록만으로는 `core:webview:allow-create-webview-window`, `core:webview:allow-create-webview`, `core:window:allow-create`, `core:app:default`가 통과했다(리뷰 재현) | 금지 목록만 | architecture.md 3.6, 리뷰 |
| `justfile`이 `TAURI_CONFIG`를 지운다(`unexport`) | Tauri는 빌드 때 이 환경 변수의 JSON을 설정에 합치므로 `capabilities` 목록을 바꿀 수 있다 | - | 리뷰 |
| 금지 권한은 architecture.md 3.6의 목록에 `core:event:default`, `core:event:allow-emit-to`를 더한 12개다. `xxx:*`는 접두어 비교다 | R-05 Q1: `core:event:default`에 `allow-emit`과 `allow-emit-to`가 들어 있다 | 문서 목록 그대로 | R-05 Q1 |
| `build.rs`는 명령 목록이 비어 있는 동안 목록을 읽지 않아도 된다 | `build.rs`는 보호 파일이고 이 작업의 범위 밖이다. 명령이 하나라도 생기면 읽지 않는 `build.rs`는 실패한다(H-13) | 지금부터 읽기 요구(지금 작업 공간이 실패한다) | architecture.md 3.6 |
| JSON(`tauri.conf.json`, capability)은 xtask 안의 작은 읽기 함수(`capability_check/json.rs`)로 읽는다. 중복 키, 뒤 쉼표, 잘못된 이스케이프는 읽기 실패(`capability:unparsable`)다 | `serde_json`을 직접 의존으로 더하면 arch-check의 `[[external-allowlist]] xtask`에 원소를 더해야 하고, "원소 하나를 빼면 사례가 깨진다" 테스트 때문에 기존 arch-check 사례도 바꿔야 한다. harness.md 3.5의 xtask 외부 의존 목록도 그대로 둔다 | `serde_json` 직접 의존 | harness.md 3.5 |
| 예제는 `harness/sql-check/`(문장·소스·인식 사례와 금액 조각 목록), `harness/migration-check/cases/<사례>/`(`store-sqlite` 폴더 모양: `migrations/`와 `migrations.lock`), `harness/capability-check/`(정상 `base/` 위에 덮는 `overlays/`, 지울 파일 목록, 사례별 허용 표)에 둔다. 테스트는 보고된 규칙 집합이 기대와 같은지 보고, 코드의 규칙 정의에서 만든 목록에 예제 없는 규칙이 있으면 실패한다 | ADR 0008과 같은 구조 | - | harness.md 6.8, ADR 0008 |

## 결과

- 실제 작업 공간의 `tauri.conf.json`이 `capabilities: ["main"]`이 됐고 `capabilities/main.json`, `permissions/commands.toml`, `ipc/commands.txt`가 생겼다. `permissions/`가 생겨 앱 ACL 매니페스트가 켜졌으므로, 권한을 받지 않은 앱 명령은 WebView에서 거부된다. CSP와 `freezePrototype`은 U-03 몫이라 그대로다.
- H-13의 `gen-bindings`는 `ipc/commands.txt`와 `permissions/commands.toml` 두 파일을 같은 목록에서 만들어야 하고, `build.rs`는 `ipc/commands.txt`를 읽어 `AppManifest::commands`에 넘겨야 한다. `build.rs`의 다른 문자열(`cargo:rerun-if-changed=…` 등)은 명령 이름 모양이 아니면 통과한다.
- harness.md 7.1의 보호 목록에서 손으로 쓰는 `capabilities/main.json`은 보호하고, 생성되는 `ipc/commands.txt`와 `permissions/commands.toml`은 에이전트 쓰기 허용 목록에 넣어야 한다(H-15).
- 원리상 놓치는 것: SQL은 암시적 별칭(`SELECT amount a … ORDER BY a`), CTE 열 이름 목록(`WITH t(a) AS (SELECT amount …)`), 이름 규칙에 맞지 않는 금액 열(`px`, `krw`), 금액 열을 하위 쿼리 결과 열 이름 없이 꺼내는 것, `String::push_str`·`+`로 문장 조각을 런타임에 붙이는 것, `execute("COMMIT")`처럼 다음 낱말 없는 문장을 못 본다. 마이그레이션은 잠금 파일과 마이그레이션 파일을 함께 고치는 것을 못 막는다. 잠금 파일은 보호 경로가 될 파일이지만 H-15·H-20의 보호가 켜지기 전까지는 리뷰에 기댄다. capability는 `build.rs`가 문자 코드나 바이트로 명령 이름을 만드는 것, 플러그인 crate가 자체 권한으로 명령을 여는 것을 못 본다.
- 금액 조각 목록, 마이그레이션 위치, 명령 목록 파일의 위치·형식은 S-15, H-13, U-03에서 다시 확인한다.
- SQL 검사는 지금 표식 수준이다(문자열이 SQL처럼 보이는지, 이름 조각). 저장소 작업 초입에 구조를 바꾼다: SQL은 정해진 파일의 상수에만 두고, 실행 메서드의 인자는 그 상수 이름만 허용하고, SQL 파서로 읽고, 금액 열 집합은 마이그레이션의 `CREATE TABLE`에서 얻고, 금액 열은 허용 위치(선택 목록, 등호 비교, 삽입 값)에만 둔다. 그때 금지 함수·이름 조각·조립 목록을 늘리는 대신 이 구조로 대체한다.
- H-13으로 넘긴 것: `build.rs`의 명령 목록 판정을 `invoke_handler`에 등록한 명령 = 명령 목록 = 권한 묶음의 대조로 바꾼다.
- 바뀌면 다시 볼 조건: Tauri의 권한 묶음·설정 병합 규칙이 바뀔 때, `store-sqlite`의 실제 스키마가 이름 규칙과 어긋날 때.
