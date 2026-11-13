# ADR 초안: D-05 보유와 포트폴리오 평가

상태: 초안. 기준 6a6acdc12a5ed53ee522fffab2a3833cf2a9a461.

## 데이터와 계산

Holding은 조회로 시장이 확정된 ResolvedOrderInstrument, Quantity, AcquisitionPrice, 선택적 현재 Price, 선택적 ReportedHolding, 수집 Timestamp를 보존한다. AcquisitionPrice는 비음수 Money를 감싼다. 무상 취득 0을 허용하되 주문 Price의 양수 계약은 바꾸지 않는다. 평균가는 토스 averagePurchasePrice 입력이고 재계산하지 않는다. 수량은 Quantity가 음수를 거부한다. 시장과 통화는 KR/KRW, US/USD를 요구한다.

평가액은 Price × Quantity, 원가는 AcquisitionPrice × Quantity, 손익은 평가액 − 원가, 수익률은 손익.checked_ratio(원가)다. 기존 exact_arithmetic::multiply와 Money의 정확 합·차만 사용한다. 새 Decimal 연산과 수량 합·차는 필요 없다. 원가 0이면 수익률은 ZeroCost로 정의되지 않음을 보존한다. 수량 0이면 시세 없이도 평가·원가·손익 0이다.

ACC-6과 R-11 Q1에 따라 표시와 합산의 정본은 토스 marketValue.amount, profitLoss.amount, profitLoss.rate다. 계산값은 별도로 보존하고 차이를 Comparison으로 공개한다. 토스 값이 없으면 계산값으로 평가한다. 계산 오류·현재가 누락은 토스 값이 있어도 평가 불가다. 현재가 없는 거래정지 보유를 0으로 숨기지 않는다. amountAfterCost의 화면 선택은 ACC-6의 T-13 실측 사항이며 이번에는 세전 amount를 명시적으로 쓴다. 평균가로 만든 원가가 토스 purchaseAmount와 일치한다는 주장은 하지 않는다.

OpenAPI HoldingsItem의 quantity, lastPrice, averagePurchasePrice, marketValue와 profitLoss는 필수이며 nullable이 아니다. domain의 None 시세는 외부 응답 null의 묵인용이 아니라 시세를 얻지 못한 재계산 입력을 표현한다. Price.krw는 필수이고 usd는 nullable(해외 보유 없음)이다. BuyingPowerResponse.cashBuyingPower와 SellableQuantityResponse.sellableQuantity도 필수·비nullable이다. 예수금 필드는 없다. 매도가능수량은 D-26 주문 검증에서 별도 조회한다.

## 포트폴리오와 불완전 합계

Portfolio는 최대 4096개 보유와 통화별 BuyingPower를 보관한다. 이 한도는 공식 업무 제한이 아닌 메모리·중복 검사 상한이다. 같은 시장·Symbol 중복은 거부하고 시장·심볼 순으로 정렬한다. 합산 순서와 오류로 제외되는 항목도 결정적이다. 두 통화의 매수가능금액을 반드시 명시적으로 받으므로 조회 실패를 0으로 바꾸지 않는다. 현금만 또는 빈 보유 목록은 허용한다.

각 보유의 계산, 선택한 평가액, 환산·누적 결과를 공개 접근자로 읽는다. 산술 오류·환율 없음은 해당 보유만 제외한다. 통화별 합계와 원화 평가 합은 Complete/Incomplete로 구분하고, 불완전 결과에는 실패 항목이 남는다. 합산 넘침도 해당 항목을 제외하고 계속한다. 완전/불완전 합계를 나타내는 봉인된 페이로드는 각각 다른 타입이다. 오류는 입력값을 담지 않는다.

다른 통화 합산은 KrwValuation만 사용한다. USD는 Mid 환산만 허용하고 사용한 FxRate 전체를 보존한다. KRW 계산값은 crate 내부 전용 KrwValuation::from_native_krw로 구성하며 통화를 검사한다. 문자열 재파싱과 표시 반올림을 거치지 않는다. 환산 불가 종목은 원통화 합에는 남을 수 있으며 원화 합만 Incomplete가 된다. 누락 통화를 원통화 합에서도 지우지 않는다.

원화 합은 ADR 0031처럼 외화 환산이 있으면 Approximate, KRW만 있으면 Exact다. 총자산은 보유 평가액 + 통화별 매수가능금액이며, 원화만 있거나 보유가 없어도 반드시 Approximate다(I-05). 총자산의 Incomplete는 근사와 별개의 축이다. U-09/A-08은 불완전 합계에 일부 보유 제외 사실을 표시해야 한다. design은 세부 문구를 정하지 않았으므로 구체 문구는 후속 messages.ts에서 정한다.

## 범위와 후속

오늘 손익은 U-09의 별도 표시 요구이고 OpenAPI dailyProfitLoss가 제공된다. 이번 D-05의 보유 평가손익과 합치지 않는다. 자체 계산에는 전일 기준가가 필요하므로 D-11 Quote 병합 뒤 현재 Price 입력을 연결하는 어댑터와 별도 일간 손익 모델을 추가한다. 토스 일간 값 보존 역시 그 모델의 일이다. D-06 비중 및 최대 잔여법, D-07 쏠림, D-28 로트·세금과 평단 계산, D-31 과거 환차손익 분해는 이번에 구현하지 않는다.

## R-11 대조 범위

| 예제 | D-05 관련 입력 | 기대값 | 테스트 |
|---|---|---|---|
| 2 | (수량, 입력 평단) 10/100,20/110,15/110,20/110,10/110 | 원가 1000,2200,1650,2200,1100 | r11_acquisition_snapshots |
| 3 | 100 × 72000 | 평가 7200000 | r11_products |
| 5 | 50×150,50×180,60×220 | 7500,9000,13200 | r11_products |
| 6 | 100주 평단70 현재65 | 원가7000,평가6500,손익-500 | r11_products |
| 7 | KRW10000000 + USD5000 × 1375 | 16875000 KRW | r11_mixed_total |
| 1 | 기간 수익률 | D-09 | 범위 밖 |
| 2 나머지 | 매매로 평단 변경·실현손익 | D-28 | 범위 밖 |
| 3·4·4b·5 나머지 | 비용·배당·세금 | D-28 및 비용 모델 | 범위 밖 |
| 6 나머지 | 과거 원화 취득원가·가격/환 효과 분해 | D-31·D-28 | 범위 밖 |
| 7 비중·8 | 배분·쏠림 | D-06·D-07 | 범위 밖 |
| 9·10 | 변동성·벤치마크 | D-11·D-09 | 범위 밖 |

속성 테스트는 Config::with_cases(4096)를 명시하고 작은 정수 곱·합을 독립 기준으로 사용한다. 기존 master 테스트·보호 규칙·의존성은 변경하지 않는다. 프로젝트 설정 hooks=true와 Git 훅 설치는 확인했으나 현재 세션의 프로젝트·훅 신뢰 적용 자체는 증명하지 못했으므로 직접 기준 검사와 커밋 훅으로 확인한다.

## 구현 검증 보충

총자산은 Total<AssetEstimate>다. AssetEstimate는 봉인된 원화 평가 합과 경제적 Approximate를 함께 갖고 into_parts로 함께 반환한다. Total<Money>, Total<KrwValuation>, Total<AssetEstimate> 각각 Complete(CompleteTotal<T>)/Incomplete(IncompleteTotal<T>)로 구분한다. CompleteTotal::value와 IncompleteTotal::subtotal의 이름도 구분한다. 매수가능금액 환산 실패는 buying_power_errors에 별도로 보존한다. 합산에서 제외된 종목은 ValuedHolding의 currency_sum/conversion 결과로 읽는다.

예제 2의 FIFO 잔여 원가 1150은 입력 평단115·수량10의 스냅샷으로도 고정한다. 예제 5의 수수료 포함 취득 입력 7507.50×1300=9759750, 9009×1350=12162150, 매도 평가13200×1400=18480000, 비용13.47×1400=18858을 r11_cost_conversion_inputs_and_exchange_effects로 대조한다. 세율·절사 자체를 구현했다는 뜻은 아니다. 예제 6은 r11_historical_price_and_exchange_effects에서 기존 환산 API와 두 평가 시점의 차로 총 -1250000, 가격 효과 -600000, 환 효과 -650000도 고정한다. 거래 환율과 현재 Mid 환율의 출처가 같다는 뜻은 아니며 지어낸 입력의 산술 대조다.

총자산 표지 회귀는 처음 원화만 있는 빈 포트폴리오에서 Exact가 나와 실패했고 AssetEstimate 도입 뒤 통과했다. 최초 타입 구현 전체를 테스트보다 먼저 작성한 것은 implementer의 테스트 우선 규칙을 지키지 못한 절차상 약점이다. R-11 기대값은 문서에서 직접 옮겼으며 구현 출력을 복사하지 않았다. 새 검사 끄기와 exact_arithmetic 변경은 없다.


## 커버리지 수정 회차

사용자가 fx_conversion.rs의 최소 내부 경로 추가를 허용했다. KrwValuation::zero와 from_native_krw만 pub(crate)로 추가한다. 전자는 원화 0과 빈 환율 목록을 만들며 실패하지 않는다. 후자는 검증된 Money를 받아 KRW만 허용하고 환율을 만들지 않는다. 공개 생성 경계와 Mid 전용 합산, 환율 보존 규칙은 그대로다. master의 기존 테스트 본문은 변경하지 않고 내부 생성 경계 테스트를 추가한다. exact_arithmetic.rs는 수정하지 않는다.

Holding의 시장 검사는 통화에서 기대 Market을 정해 ResolvedOrderInstrument의 시장과 직접 대조한다. 봉인된 종목에서 만들어질 수 없는 Unknown 분기를 만들지 않는다. 취득원가 곱과 평가손익 차의 실제 넘침·정밀도 손실을 고정 입력으로 검증한다. 수익률은 Result<HoldingReturn, MoneyError> 자체로 보존하고, Monetary 평가 전체로 다시 전파하는 실패 분기를 없앤다. ZeroCost와 RatioQuotient의 근사 표지는 유지한다. 초기화 실패가 사라진 Portfolio::evaluate는 PortfolioValuation을 직접 반환한다. 종목별 오류와 불완전 합계는 그 결과 안에 남는다.

TotalCompleteness 접근자로 테스트가 완전/불완전 상태를 직접 단언하며, 페이로드 접근자는 양 상태를 실제 생성해 검증한다. 테스트의 도달 불가능한 panic 분기도 제거한다. 관련 19개 테스트와 coverage-check가 통과했다. 검사 예외·끄기는 추가하지 않았다.
