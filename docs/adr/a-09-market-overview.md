# A-09 시장 개요 조회

- 상태: 초안

## 결정

기준은 904c78794d4e3d10d62777fe0b3ffe3112b9f20d다. GetMarketOverview는 명시적인 조회 목록을 순서대로 한 번씩 읽는다. 국내 지수·국채는 MarketInfo.indicator_quotes, ETF 근사는 BrokerRead.quotes, 일별 값은 MacroSeries.daily_values, 환율과 장 운영 정보는 기존 MarketInfo 포트를 쓴다. 새 포트와 가짜 메서드·의존성은 추가하지 않는다. 조회별 Result와 원 오류를 그대로 보존하여 일부 실패 뒤에도 다음 조회를 수행한다. NotConfigured·NoData·빈 성공 목록은 각각 유지한다. 저장·이벤트·재시도·주문 전송은 없다.

ETF 가격은 지수 포인트나 금리 퍼센트로 바꾸지 않고 EtfApproximation 변형에 담는다. MacroObservation의 MacroSource(UsTreasury/Fred/TossEtf)·단위·기준일·계열 식별자는 그대로 보존한다. 어떤 ETF가 어떤 지수·금리를 대신하는지의 카탈로그와 선택은 호출자의 공급 계약이다. 유스케이스는 추종 관계를 인증하지 않는다.

ACC-4의 5분은 domain Timestamp.elapsed_until과 TimeSpan 비교로 판정한다. 정확히 300초는 WithinFiveMinutes, 초과는 OlderThanFiveMinutes이며 장 운영 상태와 독립된 사실이다. 화면은 장중 여부와 함께 표시한다. 서버 시각이 없거나 미래 시각이면 Unknown이며 수집 시각으로 원 시각을 메우지 않는다. 원 서버/수집 시각은 값에 보존한다. MacroSeries와 calendar에는 원 수집 시각 필드가 없으므로 주입 Clock의 응답 수신 시각을 별도 관측 시각으로 붙인다. 환율도 원 수집 시각 대신 응답 수신 시각을 보존하며 as_of는 원 값 안에 있다.

## 정의 대기

| 기준 부분 | 이유 | 현재 결과 |
|---|---|---|
| 일별 지표의 오래됨 판정 | 거래일·발표 주기별 신선도 규칙/도메인 함수가 없음 | DailyPolicyPending, 원 기준일 보존 |
| calendar 원 수집 시각 및 오래됨 | 기존 타입에 원 수집 시각이 없음 | Unknown, 응답 수신 시각 보존 |
| ETF 추종 대상 기본 카탈로그 | 봉인된 매핑 타입이 없음 | 호출자가 명시한 QuoteInstrument만 조회 |

## 인계

- D-37/N-16: 일별 발표 주기와 휴장일을 반영한 신선도 도메인 계약, MacroObservation의 원 수집 시각.
- T-08/M-04: ETF 추종 카탈로그 검증, 포트 응답과 요청 식별자 결합, 원 시각 보존. ETF 가격을 지수·금리로 오해하지 않게 표시.
- U-02/G-07: OverviewFreshness 및 출처를 화면 DTO로 옮긴다. DailyPolicyPending은 내부 이름을 화면에 노출하지 않고 기준일을 표시한다. 장중 시세의 5분 초과 표시.

## 검증 경계

프로젝트 설정 hooks=true를 확인했으나 프로젝트 신뢰와 훅 신뢰 저장 상태 및 런타임 적용은 확인 불가다. 정상 Git 훅 설치를 확인했다. 실제 키·실제 계좌·실제 API는 사용하지 않았다. 기존 테스트 변경, 검사 끄기, 의존성 추가 없음.
