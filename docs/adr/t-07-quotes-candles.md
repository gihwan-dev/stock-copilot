# T-07 현재가와 종목 캔들

- 상태: 초안

## 결정

기존 TossAccountRead와 BrokerRead의 quotes/candles를 사용한다. 포트 서명은 바꾸지 않는다. 메인의 추가 승인에 따라 account_ports.rs의 두 메서드 본문만 새 market_read 모듈 호출로 바꾼다. transactions와 T-11 메서드는 보존한다. lib.rs 끝에 모듈 선언 한 줄을 추가하고, 캔들 구현은 market_read/candles.rs로 분리한다. 기존 TokenManager.get → TossHttpClient → 그룹 제한기만 사용한다. 별도 HTTP 클라이언트·주문 경로·사용자 설정은 추가하지 않는다.

quotes는 입력 1~200개를 한 배치로 받고, 송신 심볼은 중복 제거하여 결정적 순서로 한 번씩 보낸다. 결과의 입력 순서·중복 슬롯과 QuoteInstrument는 보존한다. 서버 응답을 심볼로 결합한다. 없는 종목·잘못된 개별 가격/통화/시각은 해당 슬롯의 오류이며, 배치 형태·미요청 심볼·중복 응답 행 오류와 HTTP 오류는 바깥 오류다. 타임스탬프 null은 None이다. 오래됨 판단은 하지 않는다. 현재가는 기존 Price 양수 계약을 따른다. 이번 작업의 미확정 필드 닫힌 실패 지시로 0도 거부한다. ADR 0035의 Quote 0→None과 충돌하며 ADR 0102는 보유 조회의 결정이므로 Quote 정책의 정식 변경 근거로 취급하지 않는다. 실제 의미는 T-13에서 확인하고 메인이 확정 ADR을 정리한다.

캔들 수정 여부는 호출자의 PriceAdjustment를 요청 adjusted=true/false와 각 Candle에 그대로 보존한다. AdjustmentBasis는 ADR 0035대로 동일 수정주가 요청 묶음의 맥락이다. 서버가 과거 기준일의 수정 계수나 배당 반영을 인증했다고 주장하지 않는다. 숫자는 생성 타입 검사 후 domain NumericInput·Price·Quantity·Ohlcv 생성자로만 변환한다. 어댑터에서 금액 산술을 하지 않는다.

일봉 확정 여부는 with_candle_sessions로 주입된 domain MarketSession을 같은 시장·거래일로 찾아 마지막 세션 종료와 수집 시각을 대조한다. 중복 날짜/시장, 장 운영 정보 없음, 휴장일에 반환된 가격 봉은 실패다. 로컬 장 운영 정보 부재는 Unsupported(DailyCandles), 중복·모순된 자료는 InvalidResponse(Validation)로 구별한다. 주입 API는 내부 조립 경계이며 사용자 설정이 아니다. 기본 생성자는 장 운영 정보가 없는 상태이므로 일봉은 실측/조립 공급 전까지 닫힌 실패다. 분봉은 스펙의 봉 종료 시각이 수집 시각 이하인 봉만 Finalized로 만든다. 미래 종료 시각은 이번 작업의 엄격한 거부 요구를 적용하며 진행 중 분봉의 시계 여유를 정상 자료로 승격하지 않는다.

분봉 간격은 정확히 1분이어야 한다. 장중 무체결·종가 단일가 구간과 장을 넘는 분봉 공백의 거래 없음 여부를 확인할 근거가 없으므로 공백은 실패다. 일봉 사이의 날짜 공백은 누락 날짜마다 명시적으로 주입된 같은 시장의 휴장 정보가 있어야 허용한다. 고정 24시간을 더한 절대 시각으로 오른쪽 봉과 비교하지 않고 각 봉이 보존한 현지 달력 날짜를 비교한다. 따라서 DST 해제·시작의 25시간·23시간 날짜 경계를 휴장일로 오인하지 않는다. 거래일 누락과 캘린더 부재를 주말이라고 추정하지 않는다. 이 검증은 수신 봉 사이의 간격을 검증하며, 서버가 반환하지 않은 요청 창 양끝의 거래 존재를 인증하지 않는다.

수정주가 일봉의 하루 변화 검사는 domain PriceLimits.check_adjusted_move에서 수행한다. 호출자가 제공한 기존 MasterInstrumentFacts에서 QuoteInstrument 전체가 일치하는 마스터를 찾고, 시간 오름차순의 인접 봉마다 전일 수정 종가를 기준으로 현재 수정 저가·고가를 검사한다. 가격 계산과 비교는 domain에만 있다. Decimal 곱셈으로 상하한을 반올림하거나 수익률을 나누지 않고 기존 확장 정밀도 정수 곱 비교를 재사용하므로 최소 단위·최대 금액에서도 경계를 정확히 구별한다. 허용 경계는 포함한다. 첫 봉은 다음 봉의 기준이며 첫 봉 이전의 수익률은 검증했다고 주장하지 않는다. 빈 결과·단일 봉에는 인접 하루 변화가 없다.

국내 정책은 2015-06-15 이후 KOSPI/KOSDAQ의 알려진 STOCK·Active·일반 매매 종목에 한정한다. 이전 날짜, 다른 국내 시장, 미지 종목 유형, 상장 예정/폐지, 정리매매 또는 국내 상세 정보 부재에는 기본 비율을 추정하지 않고 Unsupported(AdjustedHistory)로 닫는다. 확인한 일반 주식 정책의 전일 수정 종가 대비 0.7~1.3 바깥 저가·고가는 Validation 오류다. 미국은 기존 domain PriceLimits가 표현하는 Unlimited 정책을 유지한다. 통화가 다르면 먼저 실패한다.

이 검사는 수정 자료의 하루 변화 이상을 차단하는 적격성 규칙이며 실제 거래소 주문용 상하한을 계산하는 API가 아니다. 거래소 상하한의 호가 단위 절사나 신규 상장·재상장·기준가 예외를 추정하지 않는다. 예외일에 큰 변동이 정상이라도 보수적으로 계산 입력에서 제외될 수 있다. 이를 허용하려면 확인한 역사 사건과 domain 정책부터 별도 결정한다. 조정 계수·배당·분할 반영의 진위까지 인증하지 않는다. 분봉에는 ADR 0035의 하루 수익률 의무를 확장하지 않는다. 분봉 이상 방어 정책은 D-27/T-13 인계에 명시한다.

정책 근거는 [KRX 유가증권 가격제한폭 및 변경 이력](https://regulation.krx.co.kr/contents/RGL/03/03010100/RGL03010100T5.jsp), [KRX KOSDAQ 가격제한폭](https://regulation.krx.co.kr/contents/RGL/03/03020201/RGL03020201.jsp), [KRX 기준가와 예외](https://global.krx.co.kr/contents/GLB/06/0602/0602010201/GLB0602010201T6.jsp)다. 일반 주식 비율과 시행일·기준가 예외를 대조했으며 전체 과거 규정이나 실제 서버 자료를 검증했다는 뜻은 아니다.

2차 리뷰에서 공급 공백이 확인된 with_adjusted_candle_limits와 별도 역사 PriceLimits 저장·공급 계약은 제거했다. 당일 /price-limits 값을 과거에 복사하거나 같은 수정 기준이라고 이름만 붙이지 않는다. 기존 마스터와 장 운영 정보가 주입된 합성 수정 일봉은 별도 상하한 주입 없이 성공한다. 실제 역사 장 운영 정보의 앱 조립 연결은 아직 없으므로 기본 생성자가 임의 일봉을 성공시킨다고 주장하지 않는다. capabilities는 구현된 네 기능을 표시하며 해당 호출의 사실 공급 완료를 보증하지 않는다. 로컬 근거 부재는 응답 형식 오류와 구별한다.

요청 창 밖의 행도 시각 형식·미래·주기 정렬·OHLCV·통화·페이지 순서와 중복을 먼저 검사한다. 반환하지 않을 행의 Finalized/InProgress를 만들지 않으므로 그 날짜의 세션은 요구하지 않는다. 반환 일봉의 상태와 반환 봉 사이의 공백에 필요한 장 운영 정보만 요구한다.

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
- T-13/D-27: 수정 1분봉의 일중 가격 이상 방어 범위와 실제 무체결·단일가 공백 표현을 확인한다. 이번 일봉 상대 변화 검사를 분봉 정책 확정으로 읽지 않는다.
- D-21/D-27: 수신 자료의 누락·정지를 적격 표본으로 사용하지 않는다. 인접 수정 일봉의 하루 변화 검사는 T-07이 domain PriceLimits를 호출하여 수행한다. 첫 봉 이전 수익률이나 실제 주문용 역사 상하한은 인증하지 않는다. 이 어댑터의 정렬·간격 검증이 실제 거래일 전체와 수정 계수의 진위를 인증하지 않는다.
- T-13: 실제 키를 사용하는 확인은 사용자가 수행한다.

## 시작과 검사

최초 기준은 4df1fac4a5b67bb2e77a948e41d470dc56cd97da다. frozen install와 lefthook install 성공. 프로젝트 설정 hooks=true는 확인했지만 프로젝트/훅 신뢰와 현재 세션 적용은 확인 불가다. 홈 설정과 자격 증명을 읽거나 수정하지 않았다.

최초 master 수신은 검사 도중 master가 전진해 기준 SHA가 최신 끝과 달라져 test-diff에서 실패했다. 예외를 쓰지 않고 그 병합만 취소했다. master409741a059fa21c6d25f1a2fd63930e78405ae52 수신은 정상 훅을 통과한6989df96d40ab89d316bb1cb675bc39b31bc3390이다. 이 시작 병합은 fast 검사였으며 전체 검사라고 보고하지 않는다.

계약 테스트를 먼저 추가해 with_candle_sessions 미정의 컴파일 실패를 확인한 뒤 구현했다. 초기 red는 런타임 실패가 아닌 컴파일 실패다. 구현 후 결과·독립 리뷰·변이·최종 검사와 SHA는 실제 실행 뒤 기록한다. 기존 테스트 수정·검사 끄기·새 의존성 추가는 없다.

## 첫 구현의 손 변형

초기 구현 fee9f89bcee4f8a02ac7a1e3146cfe7a3dd88872의 mktemp -d 복사본에서 아래 28개를 한 번에 하나씩 실제 컴파일하고 관련 계약 테스트를 실행했다. 28개 모두 컴파일 후 테스트 실패로 검출됐다. 이 초기 실행의 생존 0, 컴파일 불가 0, 시간 초과 0이다. 이후 수정 소스의 결과와 합산하지 않는다. 기준/복구 대조 실행은 각각 계약 21개가 통과했다. 원본 제품 소스를 변경하지 않고 복사본을 매회 복구했다.

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

## 1차 리뷰 수정

새 Opus 리뷰와 새 Codex 비판을 거쳤다. 비판자는 제품 경로 의존 재현 패키지에서7개 반례/대조군을 실행했다. 부모는 원본의 파일·인덱스·HEAD·브랜치 참조가 위임 전후 같은 것을 확인했다. 중요 지적2개는 부모의 새 회귀2개가 수정 전 실제 컴파일 후 실패했고 수정 후 통과했다. DST 해제 일봉과72000→36000의 수정주가 이상 자료가 그 두 회귀다.

미국 DST 시작/해제, USD·Unlimited, 일봉 InProgress/마감 정각, 동일 날짜별 상하한 경계와 범위 밖 저가/고가, 수정 기준·날짜·중복 사실 검증을 추가했다. account_ports의 두 메서드는 원래 fn → impl Future + Send 표기로 복구했다. 송신 심볼은 중복 제거하고 반환 슬롯은 그대로 보존한다.

현재가0에 대한 ADR0035와0102의 충돌은 숨기지 않는다. 이번 작업은 확정 못 하는 필드를 닫힌 실패로 다루라는 구체 지시를 적용해0·음수를 오류로 유지하고 회귀를 추가했다. 오래됨 판단이나0을 정상 가격으로 만드는 경로는 없다. 메인은 확정 ADR의 두 문장을 채택 단계에서 정리한다. 미래 분봉은 이번 작업의 구체 요구대로 거부하고 AdjustmentBasis는 요청 맥락으로 보존한다. 분봉 무체결 공백의 실제 서버 의미는 T-13 실측이다. 리뷰 지적의 원등급·비판·평가는 docs/reviews/T-07.md에 기록한다.

## 1차 수정 소스의 손 변형과 타입 경계

초기28개와 합산하지 않고, 1차 수정 2ae1b580 소스의 독립 복사본을 다시 만들었다. 아래34개 모두 실제 컴파일 후 관련 계약 시험 실패로 검출됐다. 생존·컴파일 불가·시간 초과는 각각0이다. 기준·복구 대조는 각각 계약32개 통과이며 복구 후 account_ports/account_read/market_read/candles의 원본 바이트와 일치한다. 새 DST·상하한 분기는29~34번에 포함한다.

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
| 29 | DST 날짜 비교를 절대 시각 비교로 되돌림 | 컴파일 후 검출 (exit 101) | tests::review1_dst_fall_keeps_calendar_dates |
| 30 | 수정주가 상하한 검증 우회 | 컴파일 후 검출 (exit 101) | tests::review1_adjusted_half_price_is_rejected |
| 31 | 수정 기준 일치 판정 반전 | 컴파일 후 검출 (exit 101) | tests::daily_candles_preserve_each_field_order_and_explicit_adjustment |
| 32 | 상하한 날짜 일치 판정 반전 | 컴파일 후 검출 (exit 101) | tests::review1_adjusted_limits_follow_each_distinct_trading_day |
| 33 | 저가 대신 고가를 두 번 검증 | 컴파일 후 검출 (exit 101) | tests::review1_adjusted_limits_check_low_and_high_independently |
| 34 | 고가 대신 저가를 두 번 검증 | 컴파일 후 검출 (exit 101) | tests::review1_adjusted_limits_check_low_and_high_independently |

외부 경로 의존 패키지에서 올바른 BrokerRead 호출과 Future+Send는 컴파일 성공했다. InstrumentId를 QuoteInstrument 자리에 전달, bool을 PriceAdjustment 자리에 전달, Vec<Quote>를 캔들 결과로 수신은 각각 E0308, QuoteInstrument의 비공개 필드 직접 조립은 E0451로 거부됐다. 복구한 정상 호출도 컴파일 성공이다. 네 실패 모두 기대 오류 코드를 확인했으며 네트워크나 자격 증명을 사용하지 않았다. 자료는 target/T-07/type-probes의 results.json과 사례별 로그에 있다. 이 결과가 모든 타입 우회를 검사했다는 뜻은 아니다.

## 2차 리뷰 수정

2차 독립 비판은 창 밖 일봉에 불필요한 세션을 요구하는 오류를 실행으로 확정했다. 부모도 raw 요청에 정상 창 밖 행을 더한 회귀를 먼저 실행하여 실패를 확인했다. 수정 뒤 창 안 세션만으로 두 봉이 반환되고 창 밖 행의 숫자·통화·시각·중복 오류는 여전히 거부된다.

역사 상하한 공급은 성공 경로 부재가 아닌 실제 조립의 설계 공백으로 판정됐다. 부모는 별도 상하한 없이 기존 마스터와 세션만으로 수정 일봉을 받는 회귀도 먼저 실행하여 실패를 확인했다. 이후 별도 주입 API를 제거하고 위 domain 상대 변화 검사로 바꿨다. 신규 domain API는 존재 전 컴파일 실패를 확인했으며 정상 경계·경계 밖·통화·정책 미지원·미국·시행일·확장 정밀도 극값을 독립 기대값으로 검증한다.

현재가 0 정책의 확정 문서 충돌은 메인 채택 대상으로 보존한다. 수정 1분봉 반값 수락은 비판자가 재현했지만 이번 요구의 하루 수익률 문구만으로 결함을 확정하지 못했으므로 정책 인계로 남긴다. 기존 master 테스트는 수정하지 않았다. 앞의 28개와 34개 손 변형 표는 각각 당시 소스의 역사 증거이며 아래 최종 수정본 실행과 합산하지 않는다.

## 2차 수정본의 손 변형

최종 2차 수정 소스의 mktemp -d 복사본에서 아래 43개를 한 번에 하나씩 실제 컴파일하고 관련 시험을 실행했다. 모두 컴파일 후 테스트 실패로 검출됐다. 생존·컴파일 불가·시간 초과는 각각 0이다. 기준·복구 대조는 각각 현재 계약 35개 통과이며 복구한 제품 소스 7개가 원본 바이트와 일치한다. 앞선 소스의 결과와 합산하지 않는다. 경계·전일 종가 필드·창 밖 세션·시장 정책·확장 정밀도 비교의 변형을 포함한다. 로그와 집계는 target/T-07/review2-mutation-results.json, review2-mutation-01.log~review2-mutation-43.log, review2-mutations-final.log에 남겼다.

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
| 29 | DST 날짜 비교를 절대 시각 비교로 되돌림 | 컴파일 후 검출 (exit 101) | tests::review1_dst_fall_keeps_calendar_dates |
| 30 | 수정 일봉 상대 변화 검증 우회 | 컴파일 후 검출 (exit 101) | tests::review1_adjusted_half_price_is_rejected |
| 31 | 기준 전일 종가를 시가로 교체 | 컴파일 후 검출 (exit 101) | tests::review2_relative_move_boundaries_include_exact_endpoints |
| 32 | 저가 대신 고가를 두 번 검증 | 컴파일 후 검출 (exit 101) | tests::review2_relative_guard_checks_low_and_high |
| 33 | 고가 대신 저가를 두 번 검증 | 컴파일 후 검출 (exit 101) | tests::review2_relative_guard_checks_low_and_high |
| 34 | 창 밖 봉까지 장 운영 정보 요구 | 컴파일 후 검출 (exit 101) | tests::review2_outside_daily_rows_do_not_require_extra_sessions |
| 35 | 하한 배수 0.7을 0.6으로 변경 | 컴파일 후 검출 (exit 101) | price_limits::history::tests::regular_stock_daily_moves_include_both_exact_boundaries, price_limits::history::tests::price_extremes_and_unrepresentable_fractional_bounds_are_exact |
| 36 | 상한 배수 1.3을 1.4로 변경 | 컴파일 후 검출 (exit 101) | price_limits::history::tests::regular_stock_daily_moves_include_both_exact_boundaries, price_limits::history::tests::price_extremes_and_unrepresentable_fractional_bounds_are_exact |
| 37 | 하한과 같은 가격 거부 | 컴파일 후 검출 (exit 101) | price_limits::history::tests::regular_stock_daily_moves_include_both_exact_boundaries |
| 38 | 상한과 같은 가격 거부 | 컴파일 후 검출 (exit 101) | price_limits::history::tests::regular_stock_daily_moves_include_both_exact_boundaries |
| 39 | 확인한 시장 대신 다른 시장 허용 | 컴파일 후 검출 (exit 101) | price_limits::history::tests::regular_stock_daily_moves_include_both_exact_boundaries |
| 40 | 규칙 적용 날짜 판정 반전 | 컴파일 후 검출 (exit 101) | price_limits::history::tests::regular_stock_daily_moves_include_both_exact_boundaries, price_limits::history::tests::price_extremes_and_unrepresentable_fractional_bounds_are_exact, price_limits::history::tests::historical_policy_starts_at_the_verified_rule_date |
| 41 | 전일 종가 통화 판정 반전 | 컴파일 후 검출 (exit 101) | price_limits::history::tests::us_market_is_unlimited_after_currency_validation, price_limits::history::tests::price_extremes_and_unrepresentable_fractional_bounds_are_exact, price_limits::history::tests::historical_policy_starts_at_the_verified_rule_date, price_limits::history::tests::regular_stock_daily_moves_include_both_exact_boundaries, price_limits::history::tests::unsupported_market_type_and_status_never_receive_a_default_band, price_limits::history::tests::currency_mismatch_is_rejected_before_market_policy |
| 42 | 확장 정밀도 곱 대소 비교 반전 | 컴파일 후 검출 (exit 101) | price_limits::history::tests::price_extremes_and_unrepresentable_fractional_bounds_are_exact, price_limits::history::tests::regular_stock_daily_moves_include_both_exact_boundaries, price_limits::history::tests::historical_policy_starts_at_the_verified_rule_date |
| 43 | 확장 정밀도 현재가 소수 자릿수 교체 | 컴파일 후 검출 (exit 101) | price_limits::history::tests::regular_stock_daily_moves_include_both_exact_boundaries |
