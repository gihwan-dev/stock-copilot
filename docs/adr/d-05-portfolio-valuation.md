# ADR 초안: D-05 보유와 포트폴리오 평가

상태: 초안. 최초 기준 6a6acdc12a5ed53ee522fffab2a3833cf2a9a461. 적대적 리뷰 수정 기준 60aeb79ce20a5e5b5a908bc8ff332492c7db4155.

## 보유와 정본

Holding은 ResolvedOrderInstrument, Quantity, AcquisitionPrice, 선택적 현재 Price, 선택적 ReportedHolding, 수집 Timestamp를 보존한다. 조회로 확정한 시장과 통화를 대조하고 KR/KRW, US/USD만 허용한다. Quantity는 음수를 거부하고 AcquisitionPrice는 무상 취득의 0을 허용한다. 주문 Price의 양수 조건은 바꾸지 않는다. 토스 averagePurchasePrice를 입력받으며 평단을 재계산하지 않는다.

계산값은 평가액 = 현재가 × 수량, 취득원가 = 입력 평단 × 수량, 평가손익 = 평가액 − 취득원가, 수익률 = 손익.checked_ratio(취득원가)다. 기존 정확 곱·합·차를 사용한다. 수량 0은 시세 없이 평가 0이다. 취득원가 0의 계산 수익률은 ZeroCost이며, 그 밖은 Money::checked_ratio의 Exact/Approximate와 오류를 Result에 그대로 보존한다.

ACC-6·R-11에 따라 토스 제공 평가액·손익·손익률을 각각 정본으로 선택한다. SelectedValue는 값과 TossOpenApi/Calculated 출처를 함께 준다. 토스 값이 없으면 계산값을 선택한다. 계산 실패와 토스 값 부재가 겹치면 선택 결과는 오류이고 source는 None이다. 토스 값이 있으면 현재가 누락·산술 실패에도 제공값을 선택하고, 계산 결과에는 오류, 대조에는 Unavailable을 남긴다. 원화 평가에는 토스 선택값이 들어간다. 경제적 크기를 이유로 토스 값을 잘라내거나 계산값으로 교체하지 않는다. Money의 표현 범위 안에서는 추가 경제적 상한 없이 합산하며, 차이 있는 종목 목록이 포트폴리오 경고의 근거다.

HoldingsItem의 quantity·lastPrice·averagePurchasePrice·marketValue·profitLoss·dailyProfitLoss는 필수이고 nullable이 아니다. None 현재가는 재계산 입력에서 시세 부재를 표현하며 공식 null 응답으로 주장하지 않는다. amountAfterCost를 화면 정본으로 선택할지는 ACC-6의 T-13 실측 사항이다. 이번 평가는 세전 amount·profitLoss.amount·rate를 명시적으로 사용한다.

## 표시 정밀도에서의 대조

평가액·손익·손익률마다 Comparison을 공개한다. 보고값의 원래 Decimal scale을 s라 할 때, 계산값과 보고값의 차이 절댓값이 10^-s 이하면 Matches, 초과하면 Different다. 토스의 절사·반올림 방식을 확인하지 못했으므로 마지막 자리 정확히 1단위까지 허용한다. 0단위나 2단위를 허용하는 정책이 아니다. 예: 0.10769230… 대 0.1077, 23.47022016 대 23.47은 Matches다. 1.0101 대 1.00은 Different다. trailing zero가 있는 1.00의 허용 폭은 0.01이며 1과 다르다. 저장값과 선택값을 반올림하거나 바꾸지 않는다.

exact_arithmetic::compare_reported는 두 Decimal 계수를 공통 스케일의 i128 정수로 정렬해 차이를 비교한다. 보고 스케일은 정규화하지 않는다. 지수 차이는 최대 28이고 10^28은 i128/u128 안이다. 계수 정렬이 i128을 넘으면 다른 피연산자의 정렬하지 않은 96비트 계수와 최대 10^28인 허용 폭으로 상쇄할 수 없어 Different다. 부호 있는 차의 checked_sub 실패도 허용 폭 밖이다. Decimal 반올림이나 새 Decimal 산술은 없다. 근사 몫은 기존 최대 정밀도 표현값을 비교에만 사용하며 Money로 재포장하지 않는다.

NotProvided는 보고값 없음, Unavailable은 계산 실패 또는 원가 0으로 수익률 대조 불가다. 하나라도 Different면 종목 전체 Different, 그다음 Unavailable, 그다음 NotProvided, 나머지는 Matches다. PortfolioValuation은 Different 종목의 수(HoldingCount)와 ResolvedOrderInstrument 목록을 제공한다. 토스 요약과 종목 합의 대조는 통화별·지표별 MetricTotal에서 별도로 공개한다.

## 손익률의 기준과 오늘 손익

로컬 OpenAPI 1.2.28의 ProfitLoss와 DailyProfitLoss 객체 설명은 거래 통화(currency) 기준이다. 따라서 종목별 손익률은 ReturnBasis::TradingCurrency(KRW/USD)다. 해외 종목의 종목별 rate를 원화 기준으로 바꾸지 않는다. OverviewProfitLoss.rate와 OverviewDailyProfitLoss.rate는 전체 자산을 현재 환율로 원화 환산한 기준이라고 명시되어 ReturnBasis::KrwConverted로 보존한다. 다만 서버가 쓴 환율 종류·기준 시각과 비용 포함 화면 선택은 확인되지 않았으며 T-13 실측 대상이다(R-11 위험 6).

ReportedHolding은 DailyProfitLoss의 금액과 율을 그대로 보존한다. ReportedPortfolio도 요약 일간 손익의 통화별 금액과 원화 기준 율을 그대로 보존한다. 제공되지 않은 일간 값을 만들거나 종목별 일간 율을 합산하지 않는다. domain의 선택 입력 None은 제공값이 없는 합성 입력을 뜻하며, 어댑터는 required 필드 누락을 스키마 단계에서 거부해야 한다.

## 토스 요약과 완전/불완전 합계

ReportedPortfolio는 HoldingsOverview.marketValue.amount, totalPurchaseAmount, profitLoss.amount의 통화별 Price를 받고, 제공된 요약 rate·dailyProfitLoss와 수집 시각을 보존한다. Price.krw는 필수 KRW, Price.usd는 선택적 USD다. null은 해외 보유 없음이라는 스키마 의미를 유지하고 원본 CurrencyAmounts::amount에서는 None으로 읽힌다. 합산용 평가·원금·손익에서는 이 빈 통화의 합을 0으로 해석한다. 일간 손익 원본의 null은 그대로 유지한다.

통화별 MetricTotal의 정본은 토스 요약값이다. 이 값이 있으면 종목 합이 불완전해도 선택 합은 완전하다. 계산한 종목 합(완전/불완전), 선택값과 출처, 표시 정밀도 대조 결과를 함께 제공한다. 요약이 없으면 종목 합과 그 불완전 상태를 선택한다. 투자원금·총 평가손익도 같은 통화별 선택 합을 환산한다. 원화 총평가는 요약이 있으면 요약 통화 합에서 나오고, 없으면 환산 가능한 종목만 합산한다. 한 종목의 환산 실패가 나머지 종목을 지우지 않는다.

Total<T>는 Complete(봉인된 CompleteTotal<T>)/Incomplete(봉인된 IncompleteTotal<T>)다. 일반 값과 부분 합 접근자의 이름도 value/subtotal로 구분한다. 종목별 계산·통화 합산·환산 결과를 보존하고, 현금 오류는 통화와 함께 별도로 보존한다. 불완전 값에 완전 표시를 붙이지 않는다.

KrwValuation만 다른 통화 합산을 맡는다. USD에는 Mid 환산만 허용하고 쓴 FxRate 전체를 보존한다. crate 내부 zero는 실패 없는 KRW 0이고 from_native_krw는 KRW 통화를 검사한다. Buy 결과를 공개 합산 타입에 넣는 경로는 추가하지 않는다. 환산이 없으면 KrwValuation은 Exact다.

총자산은 보유 선택 평가액 + 통화별 매수가능금액이다(I-05). AssetEstimate::into_parts는 (Money, 사용한 Vec<FxRate>, 항상 Approximate)를 한 번에 돌려준다. 안쪽 KrwValuation을 노출하지 않으며 assets_accuracy 중복 접근자는 없다. 음수 매수가능금액은 그 통화의 BuyingPower 오류로 보존한다. 포트폴리오 전체 생성은 막지 않고 해당 현금만 합산에서 제외해 총자산을 Incomplete로 만든다.

## 경계와 후속

- 보유 목록은 최대 4096개다. 공식 토스 업무 상한이 아닌 자원 제한이다. 같은 시장·Symbol 중복은 거부하고 시장·심볼 순으로 정렬한다. 중복 검사는 대소문자를 구분한다. 심볼 정규화는 ADR 0022에 따라 어댑터가 맡는다.
- 보유·현금·요약·환율의 조회 시각을 서로 맞추지 않는다. 보유 원본을 ValuedHolding::holding으로, 두 현금을 PortfolioValuation::buying_power로, 요약 시각을 reported로, 실제 쓴 환율 종류·시각을 환산 분해 결과로 읽을 수 있다. 표시와 시각 정합 검사는 A-08이 맡는다.
- D-11 Quote 연결 지점은 HoldingObservation의 현재 Option<Price>다. 더 최신 Quote로 토스 현재가를 덮는 규칙은 미해결이다. 현재 토스 제공값은 계산 불가와 별개로 정본이다.
- 오늘 손익 합계의 계산은 담당 작업 없음, 메인이 docs/tasks.md에 정할 것. 이번에는 토스가 제공한 일간 값만 보존한다.
- 비중과 정수 기반 최대 잔여법은 D-06, 쏠림은 D-07, 로트·세금·평단 재계산은 D-28, 과거 스냅샷은 D-31이다.

## 검증

R-11 대조는 r11_acquisition_snapshots, r11_products, r11_mixed_total, r11_cost_conversion_inputs_and_exchange_effects, r11_historical_price_and_exchange_effects에 유지한다. 예제 2의 원가 1000/2200/1650/2200/1100·FIFO 잔여 1150, 예제 3의 7200000, 예제 5의 7500/9000/13200 및 환산 9759750/12162150/18480000/18858, 예제 6의 -1250000=-600000-650000, 예제 7의 16875000을 고정한다. 세율·로트 처리·비중 전체를 구현한 뜻은 아니다. 예제 1·4·4b·8·9·10의 기간 성과·세금·쏠림·벤치마크는 해당 후속 범위다.

속성 테스트는 Config::with_cases(4096)를 명시한다. 순서 불변·실패 종목 제외·통화별 합과 총액 외에도 소수 수량·평단·현재가·토스 보고 여부·환율을 생성하고, 독립 정수 계수로 원가·평가·손익·환산값을 대조한다. 1/3 기대값은 구현 출력 문자열 대신 분수의 정수 잔차와 반 단위 오차 한계로 검사한다. 테스트 우선 규칙을 지키지 못한 최초 회차는 절차상 약점이며, 이번 리뷰 재현 테스트는 구현 전에 추가해 컴파일 실패부터 확인했다.

새 의존성·검사 예외·끄기는 없다. master의 기존 테스트 본문은 변경하지 않는다. 프로젝트 설정 hooks=true와 Git 훅 설치는 확인했으나 세션 신뢰 적용을 증명한 것은 아니므로 직접 검사와 커밋 훅 결과로 확인한다.
