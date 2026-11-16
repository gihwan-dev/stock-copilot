# T-07 현재가와 종목 캔들

- 상태: 초안

## 결정

기존 TossAccountRead와 BrokerRead의 quotes/candles를 사용한다. 포트 서명은 바꾸지 않는다. 메인의 추가 승인에 따라 account_ports.rs의 두 메서드 본문만 새 market_read 모듈 호출로 바꾼다. transactions와 T-11 메서드는 보존한다. lib.rs 끝에 모듈 선언 한 줄을 추가하고, 캔들 구현은 market_read/candles.rs로 분리한다. 기존 TokenManager.get → TossHttpClient → 그룹 제한기만 사용한다. 별도 HTTP 클라이언트·주문 경로·사용자 설정은 추가하지 않는다.

quotes는 1~200개를 한 배치로 보내고, 입력 순서·중복 슬롯과 QuoteInstrument를 보존한다. 서버 응답을 심볼로 결합한다. 없는 종목·잘못된 개별 가격/통화/시각은 해당 슬롯의 오류이며, 배치 형태·미요청 심볼·중복 응답 행 오류와 HTTP 오류는 바깥 오류다. 타임스탬프 null은 None이다. 오래됨 판단은 하지 않는다. 현재가는 기존 Price 양수 계약을 따른다. ADR 0102의 최종 인계대로 0 가격의 실제 의미는 T-13에서 확인하고, 정상 가격으로 발명하지 않는다. 이는 ADR 0035의 초기 0→None 정책과 차이가 있으므로 메인의 문서 채택 대상이다.

캔들 수정 여부는 호출자의 PriceAdjustment를 요청 adjusted=true/false와 각 Candle에 그대로 보존한다. AdjustmentBasis는 ADR 0035대로 동일 수정주가 요청 묶음의 맥락이다. 서버가 과거 기준일의 수정 계수나 배당 반영을 인증했다고 주장하지 않는다. 숫자는 생성 타입 검사 후 domain NumericInput·Price·Quantity·Ohlcv 생성자로만 변환한다. 어댑터에서 금액 산술을 하지 않는다.

일봉 확정 여부는 with_candle_sessions로 주입된 domain MarketSession을 같은 시장·거래일로 찾아 마지막 세션 종료와 수집 시각을 대조한다. 중복 날짜/시장, 장 운영 정보 없음, 휴장일에 반환된 가격 봉은 실패다. 주입 API는 내부 조립 경계이며 사용자 설정이 아니다. 기본 생성자는 장 운영 정보가 없는 상태이므로 일봉은 실측/조립 공급 전까지 닫힌 실패다. 분봉은 스펙의 봉 종료 시각이 수집 시각 이하인 봉만 Finalized로 만든다. 미래 종료 시각은 이번 작업의 엄격한 거부 요구를 적용하며 진행 중 분봉의 시계 여유를 정상 자료로 승격하지 않는다.

분봉 간격은 정확히 1분이어야 한다. 장을 넘는 분봉 공백의 거래 없음 여부를 확인할 근거가 없으므로 공백은 실패다. 일봉 사이의 날짜 공백은 누락 날짜마다 명시적으로 주입된 같은 시장의 휴장 정보가 있어야 허용한다. 거래일 누락과 캘린더 부재를 주말이라고 추정하지 않는다. 이 검증은 수신 봉 사이의 간격을 검증하며, 서버가 반환하지 않은 요청 창 양끝의 거래 존재를 인증하지 않는다.

## 요청과 합성 녹화

로컬 docs/toss-openapi-1.2.28.json만 사용한다. 실제 API·계좌·키는 사용하지 않는다. tests/market_recordings에는 origin=synthetic인 T-20 형식의 예제 사본을 둔다. 신규 시험은 T-20 검증기와 구체 생성 타입을 모두 사용하고 루프백으로 재생한다. 기존 T-20 시험과 도우미는 수정하지 않는다.

| 엔드포인트 | 포트 메서드 | 합성 녹화 | 근거 |
|---|---|---|---|
| GET /api/v1/prices | BrokerRead::quotes | t07-quote.json | krStock 예제, PriceResponse |
| GET /api/v1/candles, interval=1d | BrokerRead::candles | t07-daily.json, t07-last.json | dailyCandles·lastPage 예제, CandlePageResponse |
| GET /api/v1/candles, interval=1m | BrokerRead::candles | t07-minute.json | minuteCandles 예제, CandlePageResponse |

201개 일봉/분봉과 오류·401·429는 위 합성 행 또는 명시적 합성 오류로 루프백 시퀀스를 구성한다. 녹화에 쿼리를 적는 것만으로 요청이 검증되지는 않으므로 실제 수신 요청을 별도로 단언한다.

## 페이징 규칙

| 조건 | 동작 |
|---|---|
| 첫 요청 | count=200, before=요청 end의 명시적 UTC ISO 시각, interval·symbol·adjusted 명시 |
| 서버 정렬 | 스펙대로 엄격한 최신순이어야 함. 페이지 내부 역순·동일 시각은 실패 |
| 다음 쪽 | 서버 nextBefore 문자열을 그대로 URL 인코딩하여 전달. 이전 before보다 엄격히 과거이며 마지막 수신 봉보다 미래일 수 없음 |
| 페이지 사이 겹침 | 같은 시각·OHLCV·수정 맥락·상태인 봉만 한 개로 병합. 값이 다르면 전체 실패 |
| 반환 범위 | [start,end), 시간 오름차순 |
| 종료 | start에 도달하거나 nextBefore가 없음. 빈 페이지와 비어 있지 않은 커서의 조합은 실패 |
| 크기와 반복 | 응답 한 쪽 200개 이하, 최대 64쪽. 상한 소진은 부분 성공으로 바꾸지 않고 오류 |
| HTTP·변환 실패 | 중간 페이지를 버리거나 정상 빈 배열로 바꾸지 않고 전체 오류 |

## 실측 대기

- 수정주가의 실제 분할/배당 반영 범위와 응답에 수정 여부·계수가 없는 한계.
- 거래 정지일의 0 OHLCV 표현. 현재 Candle/Price는 별도 거래 없음 상태를 표현하지 않으므로 전체 오류로 보존한다. 0 봉을 조용히 제거하지 않는다.
- 장을 넘는 분봉의 무체결·휴장 공백과 역사 장 운영 정보의 실제 공급.
- 실제 0 현재가, 시장별 페이지 누락·중복·커서 끝·오류 응답은 실측하지 않았다.

## 인계

- T-08/T-09 및 앱 조립: 일봉 거래일의 MarketSession 공급을 연결한다. 공개 현재 장 운영 API의 세 날짜만으로 임의 역사 구간을 채웠다고 주장하지 않는다. 임의 요일로 장 운영 정보를 만들지 않는다.
- D-11/A-20 후속: 별도 거래 없음 봉을 소비자가 보존해야 한다면 Candle과 읽기 결과 계약부터 설계한다. 이번에는 포트를 바꾸지 않는다.
- D-21/D-27: 수신 자료의 누락·정지·가격제한폭 이상을 적격 표본으로 사용하지 않는다. 이 어댑터의 정렬·간격 검증이 실제 거래일 전체와 수정 계수의 진위를 인증하지 않는다.
- T-13: 실제 키를 사용하는 확인은 사용자가 수행한다.

## 시작과 검사

최초 기준은 4df1fac4a5b67bb2e77a948e41d470dc56cd97da다. frozen install와 lefthook install 성공. 프로젝트 설정 hooks=true는 확인했지만 프로젝트/훅 신뢰와 현재 세션 적용은 확인 불가다. 홈 설정과 자격 증명을 읽거나 수정하지 않았다.

최초 master 수신은 검사 도중 master가 전진해 기준 SHA가 최신 끝과 달라져 test-diff에서 실패했다. 예외를 쓰지 않고 그 병합만 취소했다. master409741a059fa21c6d25f1a2fd63930e78405ae52 수신은 정상 훅을 통과한6989df96d40ab89d316bb1cb675bc39b31bc3390이다. 이 시작 병합은 fast 검사였으며 전체 검사라고 보고하지 않는다.

계약 테스트를 먼저 추가해 with_candle_sessions 미정의 컴파일 실패를 확인한 뒤 구현했다. 초기 red는 런타임 실패가 아닌 컴파일 실패다. 구현 후 결과·독립 리뷰·변이·최종 검사와 SHA는 실제 실행 뒤 기록한다. 기존 테스트 수정·검사 끄기·새 의존성 추가는 없다.

## 손 변형

mktemp -d 복사본에서 아래 28개를 한 번에 하나씩 실제 컴파일하고 관련 계약 테스트를 실행했다. 28개 모두 컴파일 후 테스트 실패로 검출됐다. 최종 생존 0, 컴파일 불가 0, 시간 초과 0이다. 기준/복구 대조 실행은 각각 계약 21개가 통과했다. 원본 제품 소스를 변경하지 않고 복사본을 매회 복구했다.

예비 일괄 실행에서는 조기 반환 변형이 유한 응답 수를 준비한 가짜 서버의 finish를 기다리게 했다. 그 실행은 중단했고 시간 초과를 검출로 세지 않았다. 아래 최종 실행은 변형마다 관련 계약 테스트를 지정한 별도 실행이며 예비 결과와 합산하지 않는다. 로그와 실행 스크립트는 target/T-07 아래에 보존한다.

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|

| 1 | 현재가 통화 대조 반전 | 컴파일 후 검출 (exit 101) | tests::quote_preserves_instrument_currency_price_and_both_times |
| 2 | 현재가 대신 심볼 파싱 | 컴파일 후 검출 (exit 101) | tests::quote_preserves_instrument_currency_price_and_both_times |
| 3 | 현재가 경로 변경 | 컴파일 후 검출 (exit 101) | tests::quote_preserves_instrument_currency_price_and_both_times |
| 4 | 현재가 배치 쿼리 키 변경 | 컴파일 후 검출 (exit 101) | tests::quote_preserves_instrument_currency_price_and_both_times |
| 5 | 현재가 배치 상한 201 허용 | 컴파일 후 검출 (exit 101) | tests::local_batch_rejection_remains_typed |
| 6 | 현재가 정확히 200 거부 | 컴파일 후 검출 (exit 101) | tests::batch_limit_exactly_two_hundred_is_supported |
| 7 | 중복 응답 판정 반전 | 컴파일 후 검출 (exit 101) | tests::quote_preserves_instrument_currency_price_and_both_times |
| 8 | 캔들 요청 크기 199 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 9 | 일봉 주기 분봉으로 변경 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 10 | 원주가 요청 수정주가로 변경 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 11 | 수정주가 요청 원주가로 변경 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 12 | 캔들 종목 쿼리 키 변경 | 컴파일 후 검출 (exit 101) | tests::candles_exclude_the_right_endpoint_and_keep_the_left |
| 13 | 정확히 200개 응답 거부 | 컴파일 후 검출 (exit 101) | tests::two_hundred_one_candles_page_backwards_and_merge_only_equal_overlap |
| 14 | 페이지 상한 시각 판정 반전 | 컴파일 후 검출 (exit 101) | tests::candles_exclude_the_right_endpoint_and_keep_the_left |
| 15 | 페이지 내 내림차순 판정 반전 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 16 | 요청 시작 시각 제외 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 17 | 요청 끝 시각 포함 | 컴파일 후 검출 (exit 101) | tests::candles_exclude_the_right_endpoint_and_keep_the_left |
| 18 | 페이지 겹침 값 대조 반전 | 컴파일 후 검출 (exit 101) | tests::two_hundred_one_candles_page_backwards_and_merge_only_equal_overlap |
| 19 | 커서와 마지막 봉 비교 반전 | 컴파일 후 검출 (exit 101) | tests::invalid_cursors_preserve_validation_error_without_a_second_request |
| 20 | 캔들 통화 대조 반전 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 21 | 시가를 종가로 교체 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 22 | 고가를 시가로 교체 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 23 | 저가를 시가로 교체 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 24 | 종가를 시가로 교체 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 25 | 거래량을 종가로 교체 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 26 | 미래 시각 판정 반전 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 27 | 일봉 확정 시각 판정 반전 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 28 | 휴장 공백 판정 반전 | 컴파일 후 검출 (exit 101) | tests::daily_gap_requires_explicit_nontrading_dates |
