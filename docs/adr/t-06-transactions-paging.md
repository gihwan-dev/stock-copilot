# T-06 거래내역 커서 조회

- 상태: 초안

## 결정

기존 BrokerRead::transactions와 포트·관측 타입을 유지한다. TossAccountRead의 얇은 진입점에서 거래내역 모듈을 호출하고, 기존 TokenManager.get과 TRANSACTION 제한기로만 전송한다. 계좌 헤더와 거래 유형 필터를 모든 페이지에서 유지한다. 실제 API·키·계좌 자료는 사용하지 않는다. 형식 근거는 저장소 OpenAPI1.2.28이다.

완전 응답은 ADR0034가 요구하는 해당 유형의 전체 미결제까지 포함한다. API의 날짜 필터는 예상 결제일에도 적용되므로 from/to를 보내지 않고 끝 페이지까지 수신한다. 요청 필터의 최소 거래 종류를 먼저 검증한 뒤 결제 상태와 날짜를 읽고 창 밖 결제 행은 변환·격리 전에 제외한다. 미결제는 날짜와 관계없이 보존한다. 서버 순서와 동일 행의 중복 개수를 유지하며, 금액·정산 값을 재계산하지 않는다.

정상 행은 생성 Transaction 타입과 domain 생성자를 거친다. 알려진 통화의 숫자는 NumericInput·Money·Quantity·Ratio와 길이30 검사를 통과해야 한다. 날짜·시각은 기존 TossDate·TossTimestamp로 변환한다. 정상 행을 유효 배치에 넣기 전에 요청 TransactionFilter와 종류를 대조한다. 위반 행을 조용히 버리고 Complete를 만들지 않고, 그 전에 받은 유효 자료가 있으면 PageInterrupted로 보존한다. 자료가 없으면 바깥 오류다.

확실히 창 밖인 결제 행은 제외한다. 범위가 확인되거나 범위 필드 자체가 잘못되어 범위를 알 수 없는 미지 통화 문자열은 전체 행 스키마 검사보다 먼저 원문으로 격리한다. 알려진 통화의 같은 범위 필드 오류는 닫힌 실패다. 금액 길이 등 다른 위반이 겹쳐도 미지 통화 원문을 보존한다. 통화가 숫자·불린·null·객체·배열이면 코드가 아니므로 닫힌 오류다. 알려진 통화의 미지 시장·잘못된 시장 조합도 오류이며 UnknownCurrency로 바꾸지 않는다.

## 엔드포인트와 합성 녹화

| 엔드포인트 | 포트 | 녹화 |
|---|---|---|
| GET /api/v1/transactions | BrokerRead::transactions | t06-trades.json, t06-unsettledAndSettled.json, t06-cash.json, t06-dividendAndInterest.json, t06-transfer.json, t06-empty.json |

녹화6개는 tests/recordings에서 origin=synthetic으로 T-20 자동 스키마 검증·루프백 재생을 받는다. 재시작·상한·반복·오류는 transactions.rs의 합성 응답 시퀀스로 검증한다. 문서 예제의 커서를 실제 서비스에 쓰지 않는다.

## 커서와 종료 규칙

| 조건 | 결과 |
|---|---|
| 첫 페이지 | cursor 생략, limit=100 |
| 다음 페이지 | hasNext=true와 비어 있지 않은 nextCursor를 함께 요구하고 값 그대로 URL 인코딩하여 전달 |
| 빈 페이지·100개 미만 | 다음 커서가 있으면 계속 |
| 끝 | hasNext=false와 nextCursor=null을 함께 요구 |
| 해석 가능한 페이지의 메타데이터 조합 모순 | 닫힌 실패. 현재 페이지를 포함한 유효 자료가 있으면 PageInterrupted |
| 반복 커서 | 얇은 원시 페이지에서 읽을 수 있는 nextCursor를 검사하며, 끝 표시·행 목록의 형식 오류와 관계없이 전체 봉투 검증·행 변환·상한보다 먼저 판정하고 바깥 오류. 이 예외는 부분 보존의 일반 규칙보다 우선 |
| 커서 요청의400 invalid-request | 기존 read_error 매퍼의 CursorHint::Invalid인 경우에만 모든 정상 행·원본·격리·커서 집합을 폐기하고 처음부터 한 번 재시작 |
| 힌트 없는400 및 다른 필드의400 | CursorHint::Unspecified를 재시작 허가로 추측하지 않는다. 자료가 있으면 PageInterrupted, 없으면 원래 오류 |
| 두 번째 커서 오류 | 바깥 Err로 직접 종료하며 실제 BadRequest/MalformedRequest 매핑을 보존 |
| 일반 중간 전송·HTTP·파싱 실패 | 자료를 받았으면 Incomplete(PageInterrupted), 없으면 원래 PortError |
| 페이지 상한 | 페이지 요청 회차8192. 토큰·제한기 내부 재시도는 별도. 자료가 있으면 PageInterrupted, 없으면 오류 |
| 정상·격리 행 상한 | 각각4096. 넘은 페이지의 받은 자료는4096행 이하 여러 배치로 보존하고 RowLimitExceeded. 아직 받지 않은 페이지가 없다고 주장하지 않음 |

여러 종료 이유가 겹치면 RowLimitExceeded, PageInterrupted, QuarantinedRows 순으로 정하되, 반복 커서와 재시작 소진의 바깥 오류는 사용자 브리프의 명시적 우선 예외다. 반복 커서만 행 수신 전에 검사하고, 나머지 메타데이터 조합은 유효 행을 축적한 뒤 확인해 현재 페이지 자료를 버리지 않는다. 정확히4096행은 완전 응답이 될 수 있다. 모든 페이지·필터·전체 미결제·격리 없음이 확인된 경우에만 기존 CompleteTransactionResponse 생성자를 호출한다.

## 의존성과 원본 보관

| 패키지 | 버전 | 기능 | 새 전이 의존성 | 라이선스 |
|---|---|---|---|---|
| serde_json | =1.0.154 | 기존 패키지의 raw_value만 추가 | 0개 | MIT OR Apache-2.0 |

## 원본 보관과 A-23 인계

serde_json1.0.154의 기존 패키지에 raw_value 기능만 켰다. 새 외부·전이 패키지는 없다. ParsedTransaction은 domain Transaction과 받은 철자·공백·이스케이프를 유지하는 소유 RawValue를 함께 가진다. 페이지 수집과 재시작도 이 결합을 유지한다. project는 domain 행들과 원본 조각들을 함께 반환하므로 어댑터 내부에서 양쪽에 접근할 수 있다. 현재 포트에는 정상 원본을 담을 필드가 없어 최종 투영에서 출력하지 않는다.

미지 통화 원문은65536바이트 이하의 UTF-8 경계 조각으로 UntrustedText에 보존한다. 조각 재결합은 받은 거래 행의 원문과 같아야 한다. 전체 응답·헤더·키를 복사하지 않고 해당 행만 보존하며 Debug에 원문을 노출하지 않는다.

| 받는 작업 | 인계 |
|---|---|
| A-23 | 정상 원본을 관측·저장 포트에 싣는 계약을 T-06 병합 뒤 추가한다. ParsedTransaction.source와 project의 원본 벡터를 이용한다. 이번에 포트·관측 타입은 바꾸지 않았다 |
| A-23, A-07 | 중간403·재시도 소진429 등의 원인·RetryDelay를 자료와 함께 표현할 계약을 검토한다. 현재 Incomplete에는 그 필드가 없어 원인을 동시에 전달할 수 없다 |
| S-03 | 정상 행 원본 저장은 A-23 이후 연결한다. 이번 내부 보관만으로 저장소 막힘 해소나 영속 원본 보존 완료를 주장하지 않는다 |
| D-08, A-07 | 전체 미결제와 마지막 페이지 조건을 만족시키는 조회 최적화·기간 분할 정책을 확정한다. 첫 결제 행에서 보조 조회를 끝내는 방식은 완전성 계약 정리가 먼저다 |
| T-13 | 실제 연결·커서 만료/회전·계좌 형식·정산 응답, 두 종류 필터의 쉼표 퍼센트 인코딩 수락을 사용자 실측으로 확인한다. 합성 요청 문자열 단언은 실제 API 호환성 증거가 아니다 |
| T-13, 메인 | ADR0074의 정확한 data.field=cursor 힌트만 재시작한다. 힌트 없는 실제 cursorQueryMismatch의 자동 복구는 보류한다. 같은 code/message의 invalidDateRange와 구분할 계약·실측 없이는 허가 범위를 넓히지 않는다 |

## 남은 제약과 실측 대기

전체 이력은 창을 나누어 호출해도 매번 다시 순회한다.8192페이지 상한 때문에 작은 창도 불완전해질 수 있다. 독립 비판은200페이지의 제한기 대기를 가짜 시계에서66.333333466초로 관측했다. 초기5회/초의60%인3회/초가 적용된 값이며 실제 네트워크 시간은 포함하지 않는다. 이 비용은 정확성 결함으로 확대하지 않고 잔존 설계 제약으로 남긴다.

페이지 간 결제에 대한 서버 스냅샷 보장은 없다. 마지막 페이지 수신을 한 시점의 일관된 거래 집합이라고 주장하지 않는다. 문서에 없는 거래·주문 식별자를 만들지 않는다. 필수 필드·숫자·시장 조합이 확인되지 않으면 성공 값이나0으로 보정하지 않는다. 실제 커서 만료 시점, 실서비스401·429, 실제 정산 형식은 실측 대기다.

## 범위와 검증 기록

lib.rs에는 mod transaction_read 한 줄만 추가하고 행 파서는 그 하위 row 모듈에 둔다. account_ports.rs는 transactions의 얇은 호출 본문만 바꾸며 quotes·candles는 변경하지 않는다. rules.toml 등록은 필요하지 않았다. 기존 master 테스트·주석·검사 끄기·포트·관측 타입 변경은0건이다. 새 ADR 추적에 따른 보호 생성물은 공식 쓰기 스크립트로만 갱신한다.

시작 frozen install과 lefthook install은 성공했다. 프로젝트 신뢰·훅 신뢰·현재 세션 적용 여부는 확인 불가이며, 저장소 hooks=true와 실제 Git 훅 실행을 근거로 삼는다. 정상 훅의 환경 실패와 나머지 검사 결과를 구분한다. Codex R4 Git 신원 회귀 한 건이 Xcode 라이선스 미동의로 실패한 경우에만 원 지시문의 환경2 예외를 적용한다. 나머지 check-ts와 전체 본문을 직접 순서대로 통과한 뒤 해당 커밋만 LEFTHOOK=0으로 만들고 커밋 메시지·리뷰 기록에 사유를 남긴다. 이를 정상 전체 훅 통과라고 보고하지 않는다.

최초 구현의 시험 준비 코드 컴파일 오류는 동작 red 증거가 아니다. 수정 회차의 회귀 red/green, 실제 검사·변이 결과와 정확한 SHA 범위는 docs/reviews/T-06.md에 기록한다.

## 손 변형

3차 수정 소스의 mktemp -d 복사본에서 한 번에 하나씩 실제 컴파일·실행했다. 각 변형 제한은120초이며 아래44개가 모두 컴파일 후 검출됐다. 생존0·시간 초과0·컴파일 불가0이다. 이전 실행을 합산하지 않는다. 자동 변경분 변이의 어댑터 선택0개를 검출 성공으로 세지 않는다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | quantity -> filledPrice | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 2 | quantity -> amount | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 3 | quantity -> commission | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 4 | quantity -> tax | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 5 | quantity -> netAmount | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 6 | quantity -> exchangeRate | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 7 | filledPrice -> quantity | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 8 | filledPrice -> amount | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 9 | filledPrice -> commission | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 10 | filledPrice -> tax | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 11 | filledPrice -> netAmount | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 12 | filledPrice -> exchangeRate | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 13 | amount -> quantity | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 14 | amount -> filledPrice | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 15 | amount -> commission | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 16 | amount -> tax | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 17 | amount -> netAmount | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 18 | amount -> exchangeRate | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 19 | commission -> quantity | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 20 | commission -> filledPrice | every_transaction_field_is_preserved_independently | 컴파일 후 검출(exit 100) |
| 21 | 시작 날짜 경계 제외 | both_window_boundaries | 컴파일 후 검출(exit 100) |
| 22 | 끝 날짜 경계 제외 | both_window_boundaries | 컴파일 후 검출(exit 100) |
| 23 | 창 밖 미결제 제외 | unknown_currency_is_quarantined_and_pending | 컴파일 후 검출(exit 100) |
| 24 | 거래 종류 구분자를 세미콜론으로 변경 | partial_filter | 컴파일 후 검출(exit 100) |
| 25 | hasNext 무시 | malformed_money | 컴파일 후 검출(exit 100) |
| 26 | 빈 커서 허용 | malformed_money | 컴파일 후 검출(exit 100) |
| 27 | 첫 커서 거부 | empty_and_short | 컴파일 후 검출(exit 100) |
| 28 | 보존 배치 상한을4097로 변경 | row_limit | 컴파일 후 검출(exit 100) |
| 29 | 받은 정상 부분 행 폐기 | middle_server_failure | 컴파일 후 검출(exit 100) |
| 30 | 중단 이유를 격리 이유로 변경 | middle_server_failure | 컴파일 후 검출(exit 100) |
| 31 | 필터 밖 행 허용 | filter_violation | 컴파일 후 검출(exit 100) |
| 32 | 두 번째 커서 오류의 HTTP 매핑 소실 | second_cursor_error | 컴파일 후 검출(exit 100) |
| 33 | 다른 요청 필드 오류에서 재시작 | non_cursor_bad_request | 컴파일 후 검출(exit 100) |
| 34 | 반복 커서 검사 비활성화 후 행 상한 우선 | repeated_cursor_wins | 컴파일 후 검출(exit 100) |
| 35 | 잘못된 통화 JSON 타입을 EUR로 추측 | malformed_currency_types | 컴파일 후 검출(exit 100) |
| 36 | 미지 통화 격리 전에 전체 스키마 검사 | unknown_currency_is_quarantined_before | 컴파일 후 검출(exit 100) |
| 37 | 현재 페이지 행 보존 전에 메타데이터 거부 | inconsistent_metadata_preserves | 컴파일 후 검출(exit 100) |
| 38 | 끝 플래그와 겹친 반복 커서 허용 | repeated_cursor_is_rejected_even | 컴파일 후 검출(exit 100) |
| 39 | 격리 자료만 받은 뒤 중단하면 폐기 | interruption_preserves_quarantine_without_valid_rows | 컴파일 후 검출(exit 100) |
| 40 | 행 오류 뒤 RowLimitExceeded 우선 판정 제거 | row_limit_reason_wins_when_a_later_row_is_invalid | 컴파일 후 검출(exit 100) |
| 41 | 두 검사의 격리 행 상한 제거 | quarantine_limit_stops_before_another_get | 컴파일 후 검출(exit 100) |
| 42 | 창 제외 전에 필터 검증 생략 | out_of_window_filter_violation | 컴파일 후 검출(exit 100) |
| 43 | 힌트 없는 커서 오류에서 재시작 | unspecified_invalid_request | 컴파일 후 검출(exit 100) |
| 44 | 범위 필드 파싱 실패 시 미지 통화 원문 폐기 | unknown_currency_keeps_raw_when_scope_fields | 컴파일 후 검출(exit 100) |
