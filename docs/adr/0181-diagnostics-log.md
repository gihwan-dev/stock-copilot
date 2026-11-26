# ADR 0181: 토스 호출과 동기화 결과를 가린 진단 기록으로 남기고, 현황 오류에 거친 이유를 붙인다

- 상태: 확정 (2026-11-26). v0.4.0 실사용에서 현황이 `SyncOutcome::Failed`로만 남고 원인을 알 수 없어서 정한다.

## 배경

앱에는 기록이 전혀 없었다. 실제 토스 응답은 가짜 서버로만 검증했으므로, 응답 형식 차이(serde 해독 실패), 토큰 발급 문제, 매수가능금액·보유 필드 차이 중 무엇이 실패인지 사용자 PC에서 알 길이 없었다. 화면도 "현황을 불러오지 못했어요"만 보였다.

## 결정

1. `application::diagnostics`에 포트 `DiagnosticsSink`와 닫힌 사건 `DiagnosticEvent`(토스 호출, 응답 거절, 토큰 발급, 동기화 결과)를 둔다. 사건의 글자 값은 `DiagnosticText` 하나뿐이고, 생성자가 영숫자와 ``_-./{}`:,()<>='`` 및 공백 밖의 글자를 `?`로 바꾸고 240자에서 자른다.
2. `application`에는 전역 상태를 두지 않는다(`xtask raw-types`가 `OnceLock` 정적을 막는다). 설치 지점은 경계 crate 두 곳이다. `broker_toss::install_diagnostics`(토스 호출·응답·토큰 사건)와 `src_tauri::diagnostics_log::install`(현황·거래내역·종목 정보 동기화 결과)이고, 조립 루트 `main.rs`가 같은 싱크로 한 번씩 설치한다. `TossHttpClient`·`TokenManager`·`SyncPortfolio`·`SyncParts`의 생성자는 기존 시험이 그대로 쓰므로 바꾸지 않는다. 설치 전이나 시험에서는 기록이 버려진다. 현황 동기화 결과는 두 운영 경로가 함께 쓰는 `holdings_link::sync_with_rate`에서, 거래내역은 `scheduled_ports`의 최근 동기화 결과(저장 성공 여부)로 남긴다.
3. 값 가리기는 어댑터(`broker-toss/src/diagnostics.rs`)가 한다. 경로는 `ReadRoute`의 템플릿(`/api/v1/stocks/{symbol}/warnings`)만 쓰고 질의 값과 계좌 머리글은 쓰지 않는다. 오류 코드는 알려진 코드 이름, 요청 ID는 `DiagnosticId`가 허용한 형태만 쓴다. serde 메시지는 종류(`invalid type: string <value>, expected u64`)와 필드 이름, 줄·칸만 남기고, 값이 들어갈 수 있는 따옴표·백틱 구간과 알 수 없는 메시지의 숫자는 가린다. 본문, 토큰, 키, 계좌번호, 금액은 사건에 들어가지 않는다.
4. 파일 기록은 조립 루트(`src-tauri/src/diagnostics_log.rs`)가 한다. `~/Library/Application Support/<identifier>/diagnostics.log`에 한 줄씩 KST 시각을 붙여 쓰고, 1 MiB를 넘으면 `diagnostics.log.1`로 하나만 돌린다. 쓰기는 채널을 받는 전용 스레드가 한다(`std::sync::Mutex` 금지).
5. 마지막 현황 동기화의 거친 이유(`SyncFailureReason`: 네트워크, 서버 지연, 응답 형식, 요청 거절, 종목 정보 미준비, 접근 거부, 알 수 없음)는 같은 싱크가 메모리에 둔다. `get_portfolio`만 오류형을 `IpcSyncError { kind, reason }`로 바꾸고, `retryable`일 때만 이유를 싣는다. `IpcError`는 기존 시험이 구조체 리터럴로 만들므로 필드를 더하지 않는다.
6. 화면은 이유마다 한 문장과 기록 파일 위치를 `messages.ts`에 둔다. 이유가 없으면(앱을 막 켰거나 프레임워크 실패) "알 수 없음" 문장을 쓴다.

## 다른 선택지

- 각 생성자에 싱크 매개변수 추가: 보호된 기존 시험을 바꿔야 한다.
- `tracing` 등 기록 crate: 새 의존성이고, 값 가리기를 호출부마다 믿어야 한다.
- 실패 이유를 DB에 저장: 마이그레이션이 필요하고, 다음 동기화가 곧 다시 채운다.
