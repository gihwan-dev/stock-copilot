# ADR 초안: T-12 토스 연결 확인 명령 (`crates/toss-smoke`)

- 상태: 초안 (T-12, 번호 미부여)
- 기준 커밋: `6b8cabf3b4cab2521f5489725ffc0c63de2c6e83`

## 결정

1. **키 입력 방식 (A-2)**:
   - 새 외부 패키지·`unsafe`·자식 프로세스 실행 예외를 추가하지 않는다.
   - 키는 표준 입력(파이프·리다이렉트)으로만 받는다. 명령행 인자·환경 변수·설정 파일로 키를 받는 경로를 두지 않는다.
   - 표준 입력이 터미널(TTY)이면 바이트를 한 글자도 읽지 않고 닫힌 실패(종료 코드 `3`)로 중단하며, macOS 기본 `/bin/bash 3.2`(bracketed paste 미지원)와 `zsh 5.9` 어디에 붙여넣어도 키가 화면·셸 기록에 남지 않고 Ctrl-C 중간 중단 시 서브셸이 즉시 종료되고 터미널 되비침(`stty echo`)이 복구되며 현재 셸에 변수가 남지 않는 **한 줄 서브셸 + `trap '...' EXIT` + `IFS= read -rs`** 안내를 출력한다(`INT`/`TERM`을 트랩에 넣으면 macOS bash 3.2·zsh 5.9에서 `SIGINT`를 삼켜 중단되지 않거나 `stty -echo`가 남으므로 `EXIT`만 트랩한다). 앱 종료 확인 문구(`앱을 껐습니다`)는 안내 명령에 미리 넣지 않고 사용자가 프롬프트에 직접 입력하도록 분리한다. 안내 명령은 단일 상수(`NO_ECHO_GUIDE_COMMANDS`)에서 사용법·오류 안내 모두를 생성하고, 통합 테스트에서 독립 고정 문자열 리터럴로 검증한다.
   - 명령행 인자 오류와 표준 입력 오류는 사용자가 넘긴 원문을 절대 되돌려 출력하지 않는다.
   - 제품 CLI(`toss-smoke check`)에는 테스트용 선택지(`--local-port`, `--lock-file`)를 두지 않고 `--record <저장소_밖_파일_경로>`만 허용한다. 테스트는 `RuntimeContext` 의존성 주입으로만 루프백 서버와 임시 잠금 경로를 연결한다.

2. **실서버 진입점 경계 (B-1)**:
   - `broker_toss::TossEndpoint::production()` 호출·참조 허용 위치를 `apps/desktop/src-tauri/src/main.rs`와 `crates/toss-smoke/src/main.rs` 두 진입 파일로 정한다.
   - 실제 호스트 상수(`PRODUCTION_ORIGIN`)는 계속 `crates/adapters/broker-toss/src/endpoint.rs` 한 곳에만 둔다.
   - `crates/toss-smoke`의 나머지 모듈(`src/lib.rs` 등)과 통합 테스트(`tests/**`)에서는 `TossEndpoint::production()`을 계속 금지하고, `crates/toss-smoke/src/main.rs`에서도 호스트 리터럴을 계속 금지한다.
   - `crates/xtask/src/arch_check/source/endpoint.rs`, `harness/arch-check/cases/`, `docs/architecture.md` 4.2, `docs/harness.md` 6.2를 이 결정에 맞게 갱신했다.

3. **소스 검사 허용 범위 (C-1)**:
   - `harness/source-checks/allowlists.toml`에 `crates/toss-smoke/src/main.rs`를 `test-awareness:env::args_os`와 `clock-env:now` 두 이름만으로 등록한다.
   - 환경 변수 읽기(`var`, `var_os`, `vars`, `vars_os`, `env!`, `option_env!`)는 `crates/toss-smoke/src/main.rs`에서도 계속 전면 금지하며 위반 예제로 고정한다.

4. **인자 없는 실행 (D-1)**:
   - 인자 없이 실행(`toss-smoke`)하거나 `--help`/`-h`를 넘기면 네트워크·표준 입력·잠금 파일에 닿지 않고 사용법만 출력한 뒤 `0`으로 종료한다.
   - 실제 연결 확인은 명시적 `check` 하위 명령으로만 수행한다. 기존 `crates/xtask/src/network_sandbox_tests.rs`는 수정하지 않는다.

5. **의존성**:
   - 이미 잠금 파일과 허용 목록에 있는 `tokio`(`=1.54.1`, `default-features = false`, `features = ["rt", "time", "sync"]`), `serde_json`, `dirs`(`=7.0.0`)만 사용한다. 새 외부 패키지는 추가하지 않는다.
   - `dirs::data_dir()`은 기본 잠금 파일 경로를 찾는 데만 쓰며, 홈 아래 다른 파일을 읽지 않는다. 테스트는 `RuntimeContext.lock_path` 임시 디렉터리 주입으로 실제 홈에 닿지 않는다.
   - `harness/arch-check/rules.toml`에 `toss-smoke`의 `tokio`(`socket2`, `syn`, `proc-macro2`), `serde_json`(`syn`, `proc-macro2`), `dirs`(`syn`, `proc-macro2`, 전체 대상 metadata의 `redox_users → thiserror-impl` 경로) 전이 예외만 최소 등록한다.
   - `toss-smoke`는 `broker-toss-trade`, `order-execution`, `approval-command`에 직접·전이 의존하지 않아 쓰기(주문 생성·정정·취소) 경로가 컴파일되지 않는다.

6. **앱 실행 중 판별과 토큰 잠금 계약**:
   - 데스크톱 앱(`apps/desktop/src-tauri`)에는 아직 단일 실행 잠금이 없다(U-18 미구현). 이 작업에서는 앱을 고치지 않고 잠금 계약의 `toss-smoke` 쪽 절반을 구현한다.
   - 경로: `<app_data_dir>/dev.stockcopilot.desktop/toss-token.lock` (macOS `~/Library/Application Support/dev.stockcopilot.desktop/toss-token.lock`, Linux `$XDG_DATA_HOME/dev.stockcopilot.desktop/toss-token.lock`). 제품 CLI에는 잠금 경로 재정의 선택지를 두지 않는다(기본 경로는 `dirs::data_dir()`에서 도출하며, macOS에서는 데스크톱 앱 U-18과 동일하게 `HOME` 기준 앱 데이터 디렉터리를 사용한다. 테스트는 `RuntimeContext.lock_path` 주입으로 임시 디렉터리만 쓴다).
   - 권한 및 안전성: 상위 폴더 `0700`(신규 생성 시뿐 아니라 유효한 잠금을 잡거나 확정 생성할 때 이미 존재하는 상위 폴더도 `0700`으로 조임, 단 미인식 파일 거부 시에는 건드리지 않음), 잠금 파일 `0600`. 잠금 파일 또는 직전 상위 폴더가 심볼릭 링크이거나 하드 링크(`nlink != 1`)이면 거부(종료 코드 `8`)한다. 기존 파일 열기 후 `dev`/`ino`/`nlink` 일치로 치환 여부를 재확인하며, 파일 내용이 이 계약의 알려진 마커 4종(`desktop-app`/`toss-smoke` × `running`/`stopped`) 중 하나가 아니면(비정상 종료로 남은 0바이트·손상 잠금 파일 포함) 권한·내용을 변경하지 않고 `UnsafePath`(종료 코드 `8`)로 거부해 기존 파일 손상을 막고 빈 잠금 파일 확인 후 수동 삭제 절차를 안내한다. 잠금 경로·마커 6개 상수는 통합 테스트에서 독립 리터럴 및 `tauri.conf.json`의 `identifier`와 직접 대조한다.
   - 동시성: `std::fs::File::try_lock()` 배타 권고 잠금을 사용한다.
   - 내용 형식 (ASCII `key=value`, 최대 256바이트):
     - 데스크톱 앱 실행 중(U-18): 배타 잠금 보유 + `version=1\nowner=desktop-app\nstate=running\n`
     - 데스크톱 앱 정상 종료(U-18): 배타 잠금 해제 + `version=1\nowner=desktop-app\nstate=stopped\n`
     - `toss-smoke` 확인 실행 중: 배타 잠금 보유 + `version=1\nowner=toss-smoke\nstate=running\n` (종료 시 `state=stopped`)
   - 판별 규칙:
     - `try_lock()`이 `WouldBlock`이면 앱(또는 다른 `toss-smoke`)이 실행 중으로 보고 확인 문구가 있더라도 **무조건 거부**(종료 코드 `5`)한다.
     - 잠금을 획득했고 기존 파일 내용이 정확히 `version=1\nowner=desktop-app\nstate=stopped\n`이면 앱 정상 종료가 증명된 것으로 보고(파일 내용을 유지한 채 배타 잠금만 보유) 확인 문구 없이 진행한다.
     - 파일이 없거나 내용이 알려진 나머지 3종 마커(`owner=toss-smoke` 또는 비정상 종료 `desktop-app/running`)이면 판별 불가로 보고 닫힌 실패(종료 코드 `6`)한다. 표준 입력 3번째 줄에 `앱을 껐습니다`가 정확히 있을 때만 `owner=toss-smoke`로 잠금을 잡고 진행한다. `toss-smoke`는 `owner=desktop-app`을 쓰지 않으므로 U-18 구현 전에는 매 실행마다 확인이 필요하다.
     - 표준 출력(`stdout`) 쓰기가 실패(`EPIPE` 등)하면 오류를 삼키지 않고 실행 환경 오류(종료 코드 `9`)로 끝낸다(단계 실패 코드가 이미 있으면 그 코드를 유지한다. 단, `>&-`처럼 닫힌 fd 1은 Rust Unix 런타임의 `sanitize_standard_fds`가 `main()` 전에 `/dev/null`로 열고 `std::io::Stdout`이 `EBADF`를 성공으로 반환하는 표준 라이브러리 제약이 있어 `unsafe_code = "forbid"` 하에서는 대상 밖이다).

7. **출력·녹화 및 토큰 수명**:
   - 토큰은 메모리(`application::SecretValue`)에서만 사용하고 종료 시 버린다. 키체인·파일에 토큰을 쓰지 않는다.
   - 기본 출력은 형태 전용(`&'static str` 스키마 필드명, JSON 타입, 항목 수, OpenAPI 정적 열거값 허용 목록)이며 키·토큰·계좌번호·금액·수량·가격·보유 종목명·원시 오류 본문을 출력하지 않는다.
   - 응답 녹화는 `--record <저장소_밖_파일_경로>`를 명시할 때만 수행한다. 토큰 발급 전에 상위 폴더 정준화, 저장소 루트 및 모든 상위 `.git`/`Cargo.toml` 조상 거부, 기존 파일·심볼릭 링크 거부 후 `0600` 파일로 원자 생성(`create_new(true)`)한다.
   - 녹화 파일에는 `POST /oauth2/token`의 `access_token`을 `[REDACTED]`로 치환한 요약 객체만 기록하며, 나머지 읽기 응답과 속도 제한 헤더에도 직렬화 전후로 `client_id`·`client_secret`·`access_token`의 원문·퍼센트 인코딩(대소문자)·JSON 이스케이프 바이트가 0건인지 이중 검사한다.

## 정의 대기

문서에 아직 정해지지 않은 아래 항목은 선택지와 사용자 영향을 제안하며 메인·사용자 결정 전까지 현재 기본 동작을 유지한다.

| 항목                                  | 선택지                                                                                                                                               | 사용자 영향                                                                                                                                             | 현재 기본 동작                                              |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| 터미널 자동 비표시(A-1 대 A-2)        | **A-1**: 외부 패키지(예: `rpassword` 등) 도입으로 TTY에서 직접 숨김 입력 / **A-2**: 외부 패키지 없이 TTY 닫힌 실패 + `read -rs` 셸 안내              | A-1은 복사·붙여넣기 한 번이 줄지만 새 외부 패키지 사용자 승인이 필요하다. A-2는 외부 의존·`unsafe` 0건이며 한 줄 서브셸 명령을 그대로 붙여넣어 실행한다 | A-2 (메인 승인)                                             |
| 계좌 0건 응답 시 종료 코드            | **선택 1**: 계좌 범위 4단계를 건너뛰고 시장 조회 성공 시 종료 `0` / **선택 2**: 계좌 없음을 실패로 보고 전용 비정상 종료 코드 반환                   | 계좌가 개설되지 않았거나 권한이 비어 있는 키로 T-13을 돌릴 때 시장 API 검증 결과를 정상 완료로 볼지 실패로 볼지가 달라진다                              | 선택 1 (계좌 범위 4단계 건너뜀 명시, 시장 조회 계속 후 `0`) |
| 보유 종목 0건 시 시세·일봉 조회 종목  | **선택 1**: 공개 기본 종목(`005930`)으로 `/prices`·`/candles` 확인 / **선택 2**: 보유 종목이 없으면 시세·일봉 단계를 건너뜀                          | 빈 계좌에서 시세·일봉 스키마를 함께 확인할지 여부가 달라진다                                                                                            | 선택 1 (`005930` 대체 조회)                                 |
| 추가 읽기 단계 일부 실패 시 진행 방식 | **선택 1**: `401` 인증 실패만 즉시 중단하고, 나머지 오류는 독립 읽기 단계를 이어서 진단 후 단계별 실패 코드 반환 / **선택 2**: 첫 실패에서 즉시 중단 | 토큰이 하나뿐인 제약에서 한 번의 T-13 실행으로 모든 읽기 엔드포인트의 스키마 일치 여부를 확인할 수 있는지 달라진다                                      | 선택 1 (`401`만 즉시 중단, 나머지 계속 진단)                |

## 실측 대기 (T-13 사용자 실제 실행 확인 항목)

사용자가 T-13에서 실제 키와 `--record <저장소_밖_경로>`로 실행한 뒤 아래 질문에 답을 기록한다.

1. **(S-01 토큰)** 실제 `POST /oauth2/token` 응답의 `expires_in` 값과 `token_type`이 OpenAPI v1.2.28(`Bearer`, 초 단위 정수)과 같은가?
2. **(S-01 허용 IP·오류)** 미등록 IP 또는 잘못된 키로 실패할 때 실제 HTTP 상태 코드와 오류 본문의 `error`/`code`/`requestId`/`referenceId` 필드가 `broker-toss` 오류 매핑과 일치하는가? 허용 IP 최대 등록 개수는 몇 개인가?
3. **(S-01 계좌·보유)** `GET /api/v1/accounts`에서 종합매매(`BROKERAGE`) 외 다른 `accountType`이 내려오는가? `GET /api/v1/holdings`의 평균단가(`averagePurchasePrice`)가 토스 앱 표시와 같은 방식인가? 국내·미국 종목만 있을 때 `usd`·`tax` 필드가 `null`인지 생략되는지 실제 형태가 스펙과 같은가?
4. **(S-01 거래내역)** `GET /api/v1/transactions?limit=5`의 조회 가능 기간 상한, `nextCursor`·`hasNext` 동작, 미결제/취소 거래의 `null` 필드가 스펙과 일치하는가?
5. **(ACC-6 매수가능금액·환율)** `GET /api/v1/buying-power`(KRW/USD)와 `GET /api/v1/exchange-rate`(USD/KRW) 응답의 소수 자릿수·`rateChangeType`·`validFrom`/`validUntil`이 스펙과 같고, 보유 평가와 합산했을 때 토스 앱 총자산과 대조되는가? 1억 원 판정 기준 환율 적용 기준은 무엇인가?
6. **(S-23 시세·일봉·달력·섹터)** `GET /api/v1/prices`·`GET /api/v1/candles?interval=1d&count=5&adjusted=true`에서 배당락 전후 수정주가와 일봉 기준 시각이 스펙과 같은가? 1분봉(`interval=1m`)의 과거 조회 범위 한도는 얼마인가? `market-calendar/KR`, `market-calendar/US`, `market-indicators/prices`, `sectors`에 스펙에 없는 추가 필드나 미등록 열거값이 있는가?

## 인계

- **ADR 0088 닫힘**: [docs/adr/0088-toss-endpoint-and-http-client.md](0088-toss-endpoint-and-http-client.md)의 "추가 인계" 중 T-12/T-13 `TossEndpoint::production()` 진입점 허용 위치 결정과 문서·구조 검사 정합화는 본 ADR의 결정 B-1로 완료하고 닫는다.
- **U-18 (데스크톱 앱 메뉴 막대 상주와 단일 실행)**: 위 6절 잠금 계약의 앱 쪽 절반을 구현한다. 앱이 토스 토큰을 발급·보유하기 전에 `<app_data_dir>/dev.stockcopilot.desktop/toss-token.lock`(폴더 `0700`, 파일 `0600`, 심볼릭 링크 거부)에 `File::try_lock()` 배타 잠금을 잡고 `version=1\nowner=desktop-app\nstate=running\n`을 기록한다. 잠금이 `WouldBlock`이면 `toss-smoke` 또는 다른 앱 인스턴스가 실행 중이므로 토큰 발급을 중단하고 사용자에게 안내한다. 정상 종료 시 `version=1\nowner=desktop-app\nstate=stopped\n`을 기록한 뒤 잠금을 해제한다.
- **T-13 / T-14**: 사용자는 T-13에서 저장소 밖 경로에 `--record`로 실제 응답을 녹화하고 위 실측 대기 항목을 기록한다. T-14는 그 녹화본에서 계좌번호·실제 금액·보유 종목명·심볼 등 개인 정보를 제거한 비식별화 도구와 픽스처를 만든다.

## 손 변형 검증 (1차 구현)

작업 사본과 분리된 임시 복사본(`target/T-12/manual-mutants`, 별도 `CARGO_TARGET_DIR`)에서 기준 통과·복구 통과를 확인하며 25개 변형을 실행했다. 컴파일된 24개 중 24개 전부 테스트 실패로 검출했다(생존 0건). `M-13`(`LockGuard::Drop`에서 `self.file` 미사용으로 `dead_code` 컴파일 불가)은 검출 수에서 제외했다.

| 번호 | 파일                                             | 바꾼 것                                                     | 결과              |
| ---- | ------------------------------------------------ | ----------------------------------------------------------- | ----------------- |
| M-01 | `crates/toss-smoke/src/cli.rs`                   | `is_forbidden_secret_option` 항상 false                     | 검출              |
| M-02 | `crates/toss-smoke/src/cli.rs`                   | `parse_port` 포트 0 허용                                    | 검출              |
| M-03 | `crates/toss-smoke/src/cli.rs`                   | `help` 뒤 추가 인자 허용                                    | 검출              |
| M-04 | `crates/toss-smoke/src/cli.rs`                   | `--record` 중복 옵션 허용                                   | 검출              |
| M-05 | `crates/toss-smoke/src/input.rs`                 | 터미널 표준 입력 거부 생략                                  | 검출              |
| M-06 | `crates/toss-smoke/src/input.rs`                 | 표준 입력 8192바이트 상한 완화(+1)                          | 검출              |
| M-07 | `crates/toss-smoke/src/input.rs`                 | 자격 증명 ASCII 공백(`0x20`) 허용                           | 검출              |
| M-08 | `crates/toss-smoke/src/input.rs`                 | 앱 종료 확인 문구 임의 문자열 허용                          | 검출              |
| M-09 | `crates/toss-smoke/src/lock.rs`                  | `WouldBlock` 배타 잠금 충돌 무시                            | 검출              |
| M-10 | `crates/toss-smoke/src/lock.rs`                  | 판별 불가 기존 잠금 파일 미확인 허용                        | 검출              |
| M-11 | `crates/toss-smoke/src/lock.rs`                  | 잠금 파일 심볼릭 링크 선검사 생략                           | 검출              |
| M-12 | `crates/toss-smoke/src/lock.rs`                  | 잠금 상위 폴더 심볼릭 링크 검사 생략                        | 검출              |
| M-13 | `crates/toss-smoke/src/lock.rs`                  | `LockGuard::Drop` stopped 마커 기록 생략 (`file` dead_code) | 컴파일 불가(제외) |
| M-14 | `crates/toss-smoke/src/record.rs`                | 저장소 `.git`/`Cargo.toml` 조상 검사 항상 false             | 검출              |
| M-15 | `crates/toss-smoke/src/record.rs`                | `forbidden_roots` 포함 검사 항상 false                      | 검출              |
| M-16 | `crates/toss-smoke/src/record.rs`                | `RecordHandle` 미확정 Drop 시 임시 파일 삭제 생략           | 검출              |
| M-17 | `crates/toss-smoke/src/record.rs`                | 녹화 문자열 비밀 마스킹(`scrub_string`) 생략                | 검출              |
| M-18 | `crates/toss-smoke/src/record.rs`                | 퍼센트 인코딩 대문자 변형 마스킹 무력화                     | 검출              |
| M-19 | `crates/toss-smoke/src/shape.rs`                 | 십진수 문자열 검증 생략(과학적 표기 허용)                   | 검출              |
| M-20 | `crates/toss-smoke/src/shape.rs`                 | 성공 응답의 비어있지 않은 `error` 필드 검사 생략            | 검출              |
| M-21 | `crates/toss-smoke/src/steps.rs`                 | `401` 응답 후 후속 단계 중단 플래그 생략                    | 검출              |
| M-22 | `crates/toss-smoke/src/steps.rs`                 | `BROKERAGE` 계좌 우선 선택 생략(항상 첫 계좌)               | 검출              |
| M-23 | `crates/toss-smoke/src/steps.rs`                 | 오류 진단 ID(`requestId`/`referenceId`) 비밀 마스킹 생략    | 검출              |
| M-24 | `crates/toss-smoke/src/steps.rs`                 | 종료 코드 우선순위 역전(`Additional`이 `Holdings`보다 앞)   | 검출              |
| M-25 | `crates/xtask/src/arch_check/source/endpoint.rs` | `is_root`에서 `crates/toss-smoke/src/main.rs` 허용 제거     | 검출              |

## 손 변형 검증 (1차 리뷰 수정)

1차 리뷰 지적 수정분에 대해 별도 임시 복사본(`target/T-12/manual-mutants`)에서 7개 변형을 추가 실행했고 7개 전부 컴파일 후 테스트 실패로 검출했다(생존 0건, 누적 31/31 검출).

| 번호 | 파일                             | 바꾼 것                                                             | 결과 |
| ---- | -------------------------------- | ------------------------------------------------------------------- | ---- |
| M-26 | `crates/toss-smoke/src/lock.rs`  | 하드 링크 `nlink != 1` 검사 제거(열기 전후)                         | 검출 |
| M-27 | `crates/toss-smoke/src/lock.rs`  | `LockContent::Unrecognized` 거부 제거(임의 기존 파일 덮어쓰기 허용) | 검출 |
| M-28 | `crates/toss-smoke/src/input.rs` | 안내 명령에 `env S="$sec"` 환경 변수 키 노출 삽입                   | 검출 |
| M-29 | `crates/toss-smoke/src/input.rs` | 안내 명령에 `'앱을 껐습니다'` 사전 기입                             | 검출 |
| M-30 | `crates/toss-smoke/src/lib.rs`   | `Command::Usage` 표준 출력 오류를 성공(`0`)으로 삼킴                | 검출 |
| M-31 | `crates/toss-smoke/src/lib.rs`   | `write_reports` 표준 출력 오류를 성공(`0`)으로 삼킴                 | 검출 |
| M-32 | `crates/toss-smoke/src/cli.rs`   | 제거된 `--lock-file` 우회 선택지 재허용                             | 검출 |

## 손 변형 검증 (2차 리뷰 수정)

2차 리뷰·비판 지적 수정분에 대해 별도 임시 복사본(`target/T-12/manual-mutants`)에서 5개 변형을 추가 실행했고 5개 전부 컴파일 후 테스트 실패로 검출했다(생존 0건, 누적 36/36 검출).

| 번호 | 파일                             | 바꾼 것                                  | 결과 |
| ---- | -------------------------------- | ---------------------------------------- | ---- |
| M-33 | `crates/toss-smoke/src/lock.rs`  | `APP_DATA_DIR_NAME` 계약 상수 변조       | 검출 |
| M-34 | `crates/toss-smoke/src/lock.rs`  | `LOCK_FILE_NAME` 계약 상수 변조          | 검출 |
| M-35 | `crates/toss-smoke/src/lock.rs`  | `DESKTOP_STOPPED_CONTENT` 마커 상수 변조 | 검출 |
| M-36 | `crates/toss-smoke/src/lock.rs`  | 기존 상위 폴더 `0700` 권한 조임 생략     | 검출 |
| M-37 | `crates/toss-smoke/src/input.rs` | 안내 명령에 `INT TERM` 트랩 재삽입       | 검출 |
