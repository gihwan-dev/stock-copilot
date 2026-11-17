# ADR 0132: T-10 랭킹과 수급 동향

- 상태: 확정 (2026-11-18)

## 결정

기존 TossAccountRead의 MarketInfo 읽기 메서드를 토큰 관리자·제한기·공통 오류 변환을 통과하는 조회에 연결한다. 숫자·수량·금액의 의미와 일관성은 domain 생성자로 검증한다. float 연산은 하지 않는다. 랭킹은 요청의 시장·종류·기간·필터와 순위 빈자리 및 서버 시각 또는 Unknown을 보존하고 투자자 랭킹은 세 집단의 기준 시각을 독립 보존한다. 섹터 랭킹은 매번 카탈로그와 ID·계층을 대조하며 카탈로그 오류의 status·kind·재시도 지연을 SectorRankings에 보존한다.

국내 투자자 랭킹과 수급의 봉인된 domain 결과에는 국내 시장만 허용되지만 시장 접근자가 없었다. `InvestorTradingRankings`, `InvestorFlow`, `ProgramContext`, `SupplyContext`의 봉인된 생성자가 문서상 국내 시장 Kr을 결과에 보존하고 읽기 접근자로 노출한다. 생성자가 수용하는 시장은 넓히지 않으며 새 임의 시장 입력이나 포트 trait 서명 변경은 없다.

수급 기록은 국내 종목 또는 KOSPI·KOSDAQ을 명시적으로 확인하고 문서상 포함 날짜 cursor `until`과 `nextUntil`을 사용한다. 페이지마다 최대 100행, 최대 64쪽을 받으며 날짜 중복·역순·진행하지 않는 커서를 실패시킨다. 모든 페이지가 성공해야 날짜순으로 반환한다. 프로그램 수급은 KRX 주식 수이며 서버 시각 Unknown이다. 공매도의 비율 null과 0, 신용대주와 기관 대차는 서로 구별한다.

| 엔드포인트 | 포트 메서드 | 합성 녹화 |
|---|---|---|
| GET /api/v1/rankings | MarketInfo::rankings | tests/market_recordings/t10-rankings.json |
| GET /api/v1/rankings/investor-trading | MarketInfo::investor_trading_rankings | tests/market_recordings/t10-investor-rankings.json |
| GET /api/v1/sectors/rankings | MarketInfo::sector_rankings | tests/market_recordings/t10-sector-rankings.json |
| GET /api/v1/stocks/{symbol}/investor-trading | MarketInfo::stock_investor_flow | tests/market_recordings/t10-stock-flow.json |
| GET /api/v1/market-indicators/{symbol}/investor-trading | MarketInfo::indicator_investor_flow | tests/market_recordings/t10-indicator-flow.json |
| GET /api/v1/stocks/{symbol}/program-trades | MarketInfo::program_trades | tests/market_recordings/t10-program.json |
| GET /api/v1/stocks/{symbol}/short-selling | MarketInfo::short_selling | tests/market_recordings/t10-short.json |
| GET /api/v1/stocks/{symbol}/credit-trades | MarketInfo::credit_trades | tests/market_recordings/t10-credit.json |
| GET /api/v1/stocks/{symbol}/securities-lending | MarketInfo::securities_lending | tests/market_recordings/t10-lending.json |

## 실측 대기

합성 녹화는 저장소의 토스 문서에 근거한 계약 검사다. 실제 데이터의 지연, 기간별 cursor·공백, 장중 null, 종목별 단위와 시각은 실측하지 않았으며 문서 계약에서 벗어난 숫자·행·시각은 닫힌 실패다. 서버가 0을 준 섹터 등락률을 자료 존재나 실제 보합으로 단정하지 않는다. 종목 투자자 수급의 외국인 보유 현황과 CFD는 현재 InvestorFlow 포트가 반환하는 값에 포함되지 않는다.

T-13은 스키마에서 선택 항목인 `nextUntil`, 공매도의 `shortSellingVolumeRate`·`shortSellingAmountRate`, 신용의 `marginLoan`·`stockLoan` 키가 실제로 생략되는지 확인한다. 현재는 명시적 null이 허용된 비율과 키 누락을 구별하며, 키 누락을 자료 없음이나 마지막 쪽으로 추측하지 않고 실패한다. 수급 `nextUntil`이 마지막 행과 같은 날짜인 경우의 서버 의미도 확인한다. 명세 예시는 수급에서 이전 날짜, 지표 봉에서 마지막 봉과 같은 시각을 사용한다.

## 인계

- T-13: 실제 응답과 시간·페이지 경계 및 오류 계약을 실측한다.
- A-09/G-07/M-04: TopN, 응답 시각 Unknown, 투자자별 기준 시각과 시장 차이를 표시·판정에 반영한다.

## 검사 절차

첫 T-10 커밋 훅이 진행되는 중 domain 커버리지 실행에는 어댑터 테스트가 포함되지 않음을 확인해, 직접 시작한 그 훅만 중단하고 domain 시장 접근자 검증을 별도 테스트로 추가했다. 중단된 실행은 통과로 집계하지 않으며 수정 후 정상 훅을 다시 실행한다. 다음 정상 훅은 새 파일 스테이징 뒤 설정 재생성이 빠져 `portable-r1`에서 실패했고, 저장소 생성 스크립트를 다시 실행해 설정과 해시를 갱신했다. 이 실패도 통과로 집계하지 않는다.

## 보호 파일

- harness/arch-check/rules.toml: 이 작업의 합성 녹화 재생 시험 파일만 HTTP 시험 허용 목록에 추가한다. 생성 설정·해시는 저장소 스크립트로 재생성한다.

구현된 랭킹·수급 조회는 공통 `TossAccountRead::capabilities`에 Supported로 등록한다. 이는 각 응답에 필요한 마스터·섹터 카탈로그나 실제 자료가 이미 확보됐다는 보증이 아니다.

## 손 변형 실제 실행

`mktemp -d` 복사본에서 다음 변형을 각각 컴파일하고 표시된 테스트로 실행했다. 17개 모두 해당 테스트 실패로 검출됐다. 로그는 작업 사본의 `target/T-10/mutant-*.log`다.

| 번호 | 바꾼 것 | 잡은 테스트 |
|---|---|---|
| 1 | 거래대금 순위 종류를 거래량으로 변경 | `ranking_preserves_rank_basis_market_and_observation_time` |
| 2 | 투자주의 제외 필터 값을 포함으로 변경 | `ranking_preserves_rank_basis_market_and_observation_time` |
| 3 | 요청 건수와 같은 응답을 거부 | `ranking_preserves_rank_basis_market_and_observation_time` |
| 4 | 순위 진행 방향 판정 반전 | `ranking_preserves_rank_basis_market_and_observation_time` |
| 5 | 소분류 계층 판정 반전 | `sector_rankings_validate_catalog_and_keep_unknown_server_time` |
| 6 | 외국인 집단에 기관 집단 대입 | `investor_rankings_preserve_three_groups_sides_and_distinct_times` |
| 7 | 최종 날짜 정렬 제거 | `supply_pages_are_ordered_bounded_and_cursor_is_used` |
| 8 | 중복 날짜 커서 허용 | `supply_pages_reject_duplicate_out_of_order_and_invalid_cursor` |
| 9 | 100행을 초과로 취급 | `supply_page_accepts_100_rows_and_rejects_101` |
| 10 | 프로그램 매수량에 매도량 대입 | `supply_recordings_preserve_date_symbol_and_distinct_units` |
| 11 | 공매도 금액에 거래량 대입 | `supply_recordings_preserve_date_symbol_and_distinct_units` |
| 12 | 신용 신규 수량에 상환 수량 대입 | `credit_and_lending_preserve_distinct_units` |
| 13 | 투자자 랭킹 시장을 미국으로 변경 | `investor_rankings_preserve_three_groups_sides_and_distinct_times` |
| 14 | 종목 투자자 수급 시장을 미국으로 변경 | `investor_flows_preserve_units_dates_and_investor_groups` |
| 15 | 지수 투자자 수급 시장을 미국으로 변경 | `indicator_flow_preserves_distinct_groups_and_amounts` |
| 16 | 수급 시장을 미국으로 변경 | `supply_recordings_preserve_date_symbol_and_distinct_units` |
| 17 | 프로그램 수급 시장을 미국으로 변경 | `supply_recordings_preserve_date_symbol_and_distinct_units` |
