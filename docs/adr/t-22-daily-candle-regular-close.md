# T-22 종목 일봉의 정규장 종료와 관측 종가 증거

- 상태: 초안

## 근거

I-09, ADR 0112·0113·0127·0130·0134. T-07 종목 일봉과 D-50 평가 달력이 서로 다른 종료 시각을 쓰면 정규장이 끝난 뒤에도 평가에 필요한 당일 봉이 진행 중으로 남는다.

## 결정

`RegularClose::from_session`과 `status`가 정규장 종료와 관측 시각을 판정한다. D-50의 `MarketDay::price`와 공개 함수 `regular_session_candle_status`가 같은 판정을 쓴다. T-07 종목 일봉 변환은 후자를 호출한다. 정규장 종료 정각부터 확정이고 정규장 없는 행은 종가를 판단하지 못해 거부한다. 세션의 종류와 날짜는 T-08이 만든 `MarketSession`을 그대로 사용한다.

기존 A-16의 `execute`는 증거 없는 `observe`가 `MissingPrice`를 반환하면 domain이 증거를 요구하는 보유 종목, 가격일과 원래 관측 시각을 `ValuationCloseEvidenceRepository::observed_close_evidence`로 조회한다. 이 새 포트는 마스터의 `QuoteInstrument`와 원주가 일봉을 묶은 `ObservedCloseEvidence` 목록을 반환한다. 가짜 구현은 증거가 없으면 빈 목록을 반환하고 domain은 저장을 거부한다. 가짜 저장소는 합성 증거를 공급한다. 실제 API 호출은 추가하지 않는다. domain은 연장 거래가 있는 통화의 비0 보유마다 종목·시장·통화·식별자, 일봉·원주가·확정 상태, 가격일·수집 시각, 관측 현재가와 종가의 일치, 보유 및 통화별 보고 평가액의 정확한 일치를 검사한다. 증거는 관측 이후에 수집할 수 없다. 누락·중복·불일치 증거는 저장을 막는다. 기존 총액 비교의 허용 오차는 종가 증명에 사용하지 않는다. 확인된 증거는 스냅샷에 함께 남기고 저장 포트로 전달한다. 연장 거래가 없는 통화의 기존 관측 정책과 정확한 0 경로는 유지한다.

이 일치는 제공된 종가·마스터·관측값의 일관성을 확인한다. 원본이 진짜 정규장 종가인지 또는 수집 경로가 정본인지 자체 인증하지 않는다. 평가액이나 수량을 새로 계산하는 규칙은 추가하지 않고 기존 domain 평가 결과를 대조한다.

병합한 S-04의 SQLite `MarketStore`는 일봉을 포함한 시장 데이터 캐시를 저장하지만, 이 작업의 `ValuationCloseEvidenceRepository`를 구현하지 않는다. 조회에 쓰는 `ResolvedOrderInstrument`만으로는 마스터가 부여한 `InstrumentId`를 복구할 수 없다. 실제 연결에서는 신뢰할 수 있는 마스터에서 `QuoteInstrument`를 얻어 봉과 대조해야 한다. 평가 스냅샷과 종가 증거의 실제 영속 구현도 아직 없다. 이번 저장·재조회 검증은 메모리 가짜에 한정한다.

정규장이 끝나기 전 연장 거래 달력의 비0 보유는 전일 봉이 공급되어도 기존과 같이 `MissingPrice`로 거부한다. 증거 저장소 오류가 이 사유를 덮지 않도록 해당 통화의 가격 기준이 당일 정규장 종가인지 domain 달력에서 확인한 뒤에만 증거를 조회한다. 미국 전일 봉도 해당 시장의 당일 정규장이 끝나기 전에는 같은 거부를 유지한다.

## 미해결과 제안

일반 봉 저장과 종가 증거 공급은 별개다. 같은 종목·날짜에 수집 시각이 다른 증거가 여러 건 반환되면 종가가 같아도 다르더라도 현재는 `DuplicateInstrument`로 저장을 거부한다. 어떤 관측을 같다고 보거나 최신 관측을 선택할지는 확정된 정책이 아니다. A-16 / S-18에서 실제 저장소 연결 전에 중복의 의미를 결정할 때까지 복수를 거부하는 것을 제안한다. 단일 증거를 임의로 선택했다고 주장하지 않는다.

보고 평가액의 정밀도 차이를 종가 증거에서 어떻게 수용할지는 정의 대기다. 예를 들어 0.5주와 종가 17의 정확한 평가액은 8.5인데 제공 평가액이 9라면, ADR 0038의 표시 정밀도 대조가 `Matches`여도 이 관측은 정확한 값 차이 때문에 거부된다. 9.0은 원래 자릿수가 달라 표시 정밀도 대조도 다르다. 소수 수량 자체가 거부 조건은 아니며, 실제 토스가 이렇게 보고하는지는 확인하지 않았다. 표시용 허용 오차는 반올림 방식이나 정규장 가격의 출처를 보증하지 않으므로 종가 증거로 전용하지 않는다. 정책 결정 전에는 불일치 시 저장을 거부하는 것을 제안한다. A-16 / A-26에서 보고 정본과 계산값의 보존·출처·허용 범위를 결정한다.

정규장 종료 뒤 현재가가 없는 비0 보유에 증거가 있으면 `InvalidCandle`, 증거가 없으면 `MissingPrice`가 될 수 있으며, 증거 조회 오류가 먼저 전달될 수도 있다. 실제 소비자의 오류 분류와 재시도 우선순위는 A-26에서 정한다.

## 실측 대기

T-13: 실제 토스 일봉의 OHLC와 거래량이 정규장 이후 체결을 포함하는지 확인하지 않았다. 포함한다면 16:00에 받은 봉의 종가와 거래량은 정규장 전용 값이 아닐 수 있고, 확정 상태·동일 가격 비교만으로는 이를 구별할 수 없다. 이 경우 D-49의 평가액과 A-16의 종가 증거 주장이 실제 정규장 종가와 다를 수 있다. 추측해 가격이나 거래량을 보정하지 않는다.

반대로 원본 봉이 정규장 전용이더라도 토스의 16:00 보유 현재가나 보고 평가액이 연장 거래 가격을 반영해 정규장 종가 또는 그 재계산액과 다르면 A-16 관측은 거부되어 저장되지 않을 수 있다. 실제 제공값의 의미와 발생 빈도는 확인하지 않았다. T-13에서 일봉뿐 아니라 보유 현재가·평가액의 가격 기준과 시각도 함께 실측한다.

## 인계

| 받는 작업 | 내용 |
|---|---|
| T-13 | 실제 응답의 일봉 구성, 연장 거래 포함 범위와 날짜·수집 시각, 보유 현재가와 평가액의 가격 기준·관측 시각, 보고 정밀도와 반올림·절사 여부를 실측한다. |
| A-26 | 실제 정규장 종가의 출처와 날짜·가격을 검증해 D-49 이력과 증거에 공급하고 날짜별 성공·실패 및 재시도를 기록한다. 보고 정본과 계산값의 불일치 수용·출처와 오류 우선순위를 결정한다. |
| A-16 / S-18 | 실제 시장 데이터 저장소가 신뢰할 수 있는 마스터와 봉에서 새 증거 조회 포트를 구현하고 저장 구현의 증거 영속성과 재조회 계약을 검증한다. 보고 정본과 종가 기반 계산값의 차이도 보존·출처 정책에 반영한다. 이번 테스트의 메모리 저장소는 실제 저장 구현이 아니다. |

## 검증

합성 녹화의 정규장 15:30, 애프터마켓 20:00 사례에서 기존 구현은 15:30에 `InProgress`여서 경계 테스트가 실패했고 변경 뒤 통과했다. 양수 3주의 당일 종가 72,000원은 D-49에서 주식 216,000원·계좌 217,000원으로 계산되고 A-16 관측에 같은 종가 증거와 함께 기록된다. 15:00·정각·연장 거래, 반일장, 휴장, 정규장 누락, 미국 전일과 잘못된 증거 사례는 합성 자료로 확인한다.

변경분 자동 변이는 테스트 추가 전에 domain·application의 63개를 실행해 40개를 검출했고 7개가 생존했으며 16개는 컴파일 불가였다. 시간 제한에는 걸리지 않았다. `testkit-ports`에 종가 증거 저장·재조회, 같은 시장과 통화의 다른 심볼 거부, 분봉·수정주가·미확정 봉 거부 회귀를 추가했다. 생존한 7개는 새 테스트를 포함한 별도 사본에서 다시 각각 적용해 모두 검출했고 기준·복원 후 테스트도 통과했다. 자동 변이 전체를 다시 실행한 결과는 아니다. 빈 증거 반환의 재현은 미사용 인자 이름만 바꾸어 컴파일하며 반환 동작은 같다.

## 보호 파일

`harness/arch-check/rules.toml`의 broker-toss 시험 허용 목록에 이 작업의 새 계약 테스트 파일만 등록한다. 이 파일은 로컬 가짜 HTTP 응답과 A-16의 `execute`를 함께 호출한다. 제품 HTTP 전송 경로는 추가하지 않는다. 공식 생성기가 ADR 초안의 보호 경로를 `.codex/config.toml`에 등록하고 해당 설정의 해시를 `harness/claude-hooks/hashes.json`에 갱신한다. 생성기는 경로를 자동 열거하므로 초안 이름을 생성물에 넣지 말라는 일반 지시와 충돌한다. 생성기를 바꾸거나 보호 범위를 줄이지 않고 훅이 요구하는 결과를 따랐다.

이 예외는 해당 테스트 파일 전체에 적용된다. 나중에 그 파일에 HTTP 쓰기 호출이 추가되면 `http-write` 규칙 하나로는 탐지하지 못하므로 변경을 따로 검토해야 한다. 다른 파일과 다른 검사에는 예외를 확장하지 않는다.

이 세 파일의 변경 내용은 브리프가 미리 승인한 범위다. 그러나 이를 포함한 구현 커밋은 사용자 서명이 없다. 기준 커밋의 서명 잠금점과 서명자 목록은 비어 있으며, 실제 서명 검증을 통과했다는 뜻은 아니다. 보호 경로의 사용자 서명 요건은 내용 승인과 별개로 메인이 통합 전에 해결해야 한다.

## 손 변형

병합된 코드의 `mktemp -d` 사본에서 각 변형을 하나씩 적용하고 `cargo test --locked -p domain -p broker-toss t22_`를 실행했다. 기준과 복원 후 실행은 관련 테스트 24개가 각각 통과했다. 컴파일 가능한 변형은 30개이고 모두 아래 테스트가 검출했다. 컴파일 불가 2개는 이 수에 넣지 않는다. 자동 변이에서 생존한 7개도 아래에 별도로 실행해 검출했다.

| 번호 | 바꾼 것 | 잡은 테스트 또는 결과 |
|---|---|---|
| 1 | 종료 정각을 미확정 | `tests::t22_half_day_and_missing_regular_session_remain_distinct` |
| 2 | 종료 비교 방향 반전 | `tests::t22_evidence_repository_error_prevents_snapshot_storage` |
| 3 | 정규장 중에도 확정 | `tests::t22_intraday_previous_close_keeps_missing_price_without_querying_evidence` |
| 4 | 정규장 없는 날을 진행 중으로 수용 | `tests::t22_missing_regular_and_holiday_reject_returned_candles` |
| 5 | 정규장 대신 연장 구간 선택 | `tests::t22_multiple_close_observations_remain_ambiguous_even_when_prices_match` |
| 6 | 어댑터 일봉 상태를 항상 확정 | `tests::t22_half_day_and_missing_regular_session_remain_distinct` |
| 7 | 어댑터 일봉 상태를 항상 진행 중 | `tests::t22_us_previous_local_day_is_finalized_at_korean_16` |
| 8 | 증거 개수 상한을 0으로 축소 | `tests::t22_multiple_close_observations_remain_ambiguous_even_when_prices_match` |
| 9 | 증거 상한 정각도 거부 | `valuation_snapshot::tests::t22_exact_evidence_limit_reaches_duplicate_validation` |
| 10 | 연장 거래의 증거 검사를 건너뜀 | 컴파일 불가 (도달 불가능 코드·미사용 경고) |
| 11 | 종목 심볼 비교 반전 | `tests::t22_multiple_close_observations_remain_ambiguous_even_when_prices_match` |
| 12 | 시장 비교 반전 | `tests::t22_multiple_close_observations_remain_ambiguous_even_when_prices_match` |
| 13 | 통화 비교 반전 | `tests::t22_multiple_close_observations_remain_ambiguous_even_when_prices_match` |
| 14 | 중복 검출을 반전 | `tests::t22_multiple_close_observations_remain_ambiguous_even_when_prices_match` |
| 15 | 미사용 증거 검사 반전 | `tests::t22_evidence_query_uses_holding_observation_time` |
| 16 | 봉 식별자 검사를 반전 | `tests::t22_recorded_candle_supplies_observed_snapshot_evidence` |
| 17 | 봉 통화 검사를 반전 | `tests::t22_evidence_query_uses_holding_observation_time` |
| 18 | 일봉 검사를 반전 | `tests::t22_recorded_candle_supplies_observed_snapshot_evidence` |
| 19 | 원주가 검사를 반전 | `tests::t22_evidence_query_uses_holding_observation_time` |
| 20 | 확정 상태 검사를 반전 | `tests::t22_recorded_candle_supplies_observed_snapshot_evidence` |
| 21 | 평가 시각과 같은 수집도 거부 | `tests::t22_recorded_candle_supplies_observed_snapshot_evidence` |
| 22 | 보유 관측 시각과 같은 수집도 거부 | `tests::t22_evidence_query_uses_holding_observation_time` |
| 23 | 수집 시각의 가격일 비교 반전 | `tests::t22_evidence_query_uses_holding_observation_time` |
| 24 | 관측 가격과 종가 비교 반전 | `tests::t22_recorded_candle_supplies_observed_snapshot_evidence` |
| 25 | 개별 평가액 비교 반전 | `tests::t22_recorded_candle_supplies_observed_snapshot_evidence` |
| 26 | 요약 평가액 정확 비교 반전 | `tests::t22_recorded_candle_supplies_observed_snapshot_evidence` |
| 27 | 관측 스냅샷 증거를 버림 | `tests::t22_evidence_query_uses_holding_observation_time` |
| 28 | 관측 시각의 정규장 기준 삭제 | 컴파일 불가 (미사용 함수 경고) |
| 29 | 증거 조회 전 정규장 기준 삭제 | `tests::t22_intraday_previous_close_keeps_missing_price_without_querying_evidence` |
| 30 | 보유 관측 뒤 수집 거부 삭제 | `valuation_snapshot::tests::t22_close_evidence_must_not_postdate_the_holding_observation` |
| 31 | 비0 보유의 보고액 0 대조 삭제 | `valuation_snapshot::tests::t22_zero_report_must_match_nonzero_holdings_with_close_evidence` |
| 32 | 조회 기준을 보유 관측에서 기록 시각으로 | `tests::t22_evidence_query_uses_holding_observation_time` |
| 33 | 자동 생존 재검: 빈 증거 반환 | `tests::t22_observed_snapshot_records_the_repository_close_evidence` |
| 34 | 자동 생존 재검: 심볼과 시장의 AND를 OR로 | `tests::t22_same_market_and_currency_do_not_match_another_symbol` |
| 35 | 자동 생존 재검: 종목과 통화의 AND를 OR로 | `tests::t22_same_market_and_currency_do_not_match_another_symbol` |
| 36 | 자동 생존 재검: 일봉 조건 앞 OR를 AND로 | `tests::t22_close_evidence_requires_a_finalized_raw_daily_candle` |
| 37 | 자동 생존 재검: 원주가 조건 앞 OR를 AND로 | `tests::t22_close_evidence_requires_a_finalized_raw_daily_candle` |
| 38 | 자동 생존 재검: 확정 조건 앞 OR를 AND로 | `tests::t22_close_evidence_requires_a_finalized_raw_daily_candle` |
| 39 | 자동 생존 재검: 기록 시각 조건 앞 OR를 AND로 | `tests::t22_close_evidence_requires_a_finalized_raw_daily_candle` |
