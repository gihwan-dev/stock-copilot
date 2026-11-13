# D-04: 환율 방향과 환산 결과의 평가 맥락

- 상태: 초안
- 날짜: 2026-11-14
- 기준 커밋: cdb712fc4fab831c27fc81549372cc706acc032b
- 관련: D-04, D-05, D-31, D-32, T-08, ACC-3, ACC-6

## 근거와 종류

로컬 docs/toss-openapi-1.2.28.json의 ExchangeRateResponse, R-01 Q13, R-11 Q4, design ACC-3·ACC-6·6.3·6.5, architecture 3.4·8, ADR 0022·0025·0028·0029를 대조했다. 외부 API 호출이나 실제 계좌 접근은 하지 않았다.

| 종류 | 토스 필드 | 방향 | 쓰는 곳 |
| --- | --- | --- | --- |
| Mid | midRate | USD 1단위당 KRW | 원화 합산 기본 |
| Buy | rate | USD 1단위당 KRW | 명시적으로 선택한 매수 환율 환산. D-32의 보수적 비교 후보 |

매도 환율은 스펙에 없으므로 Sell을 만들지 않는다. basisPoint는 (rate-midRate)/midRate×10000인 상대 차이고 환율이 아니다. rateChangeType은 등락 구분이므로 종류에 넣지 않는다. 거래 환율(Transaction.exchangeRate)과 세법 환율은 R-11에서 별개로 구분하며 이번 참고 환율 API 타입에는 포함하지 않는다. 필요한 후속 작업에서 출처와 의미를 별도로 확장해야 한다.

환산은 FxPair::UsdKrw에서 USD→KRW 곱셈만 지원한다. FxPair::KrwUsd는 비교 시 다른 방향을 구별하는 메타데이터로 표현하되 환산은 UnsupportedDirection으로 거부한다. 역방향 나눗셈을 추가한 것이 아니다. convert_for_historical_valuation은 입력 Money가 base와 다르면 CurrencyMismatch다. 일반 Money의 다른 통화 합산 거부는 그대로다. convert_for_aggregation은 Mid만 허용하며 성공 결과는 봉인된 MidConvertedMoney다. Buy 결과 ConvertedMoney는 원화 평가 합에 타입이 맞지 않아 들어갈 수 없다. FxRateKind의 기본값도 Mid다.

D-32는 conservative_with로 같은 쌍의 더 큰 환율을 선택하고 convert_for_order에 판정 시각을 전달한다. 다른 통화쌍은 PairMismatch, 같은 값이면 수신자인 왼쪽 환율을 보존한다. 선택된 객체 전체(종류·출처·유효 기간)를 반환하며 새 환율을 합성하지 않는다. 최신성은 선택한 환율에 대해 확인하고, 만료한 높은 환율 대신 낮은 환율로 조용히 대체하지 않는다. 같은 응답의 rate·midRate를 묶어 공급하는 것은 T-08의 책임이다. 80% 문턱과 고액 확인은 D-32가 맡는다. R-01 Q13의 이전 95%·재시도 추천은 현행 design 6.5의 80%·재전송 금지보다 앞서지 않는다.

## 검증과 생성 경계

FxRate 필드는 비공개다. NumericInput의 복사 전 256바이트 상한과 Price의 양수 검증, D-01의 정확 정규형 파서를 재사용한다. 지수·공백·플러스·밑줄·유니코드 숫자·소수 29자리·범위 초과를 거부하고 선행·후행 0은 허용한다. 입력 오류는 값을 담지 않는 FxRateError다. 새 원시 타입 허용 파일·의존성은 없다. 환율 수치는 내부적으로 Price를 사용해 양수 및 quote 통화를 함께 보존한다. 공개 읽기 접근자도 Price다.

Money::new(Amount, Currency)는 pub(crate)로 닫는다. 공개 생성자는 Money::parse(&NumericInput, Currency)뿐이며 NumericInput을 통해 D-01 Amount::parse를 호출한다. Amount 읽기는 유지하지만 얻은 Amount를 다른 통화 Money로 조립하는 외부 타입 경로는 없다. 외부에서 문자열을 새로 만들어 파싱하는 것은 명시적 입력 경계이며 입력의 진실성을 인증하는 기능은 아니다. domain 밖 Money::new 호출처는 수정 시점에 없었다. master의 기존 테스트 본문은 변경하지 않는다.

## 유효 기간과 시각

as_of와 valid_until은 ADR 0028의 Timestamp를 가진 TimeInterval로 보존하고 출처는 FxSource::TossOpenApi라는 닫힌 열거형이다. ExchangeRateResponse의 validFrom·validUntil은 모두 required이고 type=string, format=date-time이며 nullable이 아니다. T-08 어댑터는 응답의 baseCurrency/quoteCurrency를 검증하고 TimeInterval::try_new(validFrom, validUntil)을 전달한다. 종료가 시작보다 늦지 않으면 기존 TimeIntervalError::NonIncreasing으로 구성할 수 없다.

GET /api/v1/exchange-rate의 설명은 1분 갱신과 validFrom~validUntil의 유효 시간 윈도를 명시하지만 경계의 포함 여부는 명시하지 않는다. 명세에 없는 포함 규칙을 확인된 사실로 주장하지 않는다. 인접한 1분 구간이 겹치지 않도록 ADR 0028의 시간 구간 관례와 같은 [validFrom, validUntil)을 앱 정책으로 정한다. validity_at(at)은 시작 포함·종료 제외를 판정해 FxValidity를 반환한다. 시각은 인자로만 받는다.

convert_for_order는 주어진 시각이 유효 구간 밖이면 OutsideValidity로 거부한다(D-32). convert_for_historical_valuation과 convert_for_aggregation은 만료를 강제하지 않아서 D-31 과거 스냅샷을 재계산할 수 있다. '현재 시각에서 몇 분까지 낡은 응답을 허용하는가' 같은 추가 정책은 도입하지 않는다.

## 정확 산술과 평가 근사

환산은 exact_arithmetic::multiply를 재사용한다. 새 Decimal 산술을 다른 파일에 쓰지 않는다. 표현 손실은 PrecisionLoss, 범위 초과는 Overflow이며 반올림해 Money로 바꾸지 않는다. exact_arithmetic.rs와 master의 기존 테스트를 변경하지 않는다. money.rs는 공개 생성 경로만 좁히고 파싱 생성자를 추가한다.

ConvertedMoney는 정확한 산술 결과 Money와 실제 FxRate를 비공개로 보관한다. 산술적으로 정확한 곱과 현재 참고 환율로 평가한 경제적 근사는 별개다. 일반 환산 결과의 into_parts()는 (Money, FxRate, ValuationAccuracy::Approximate)를 함께 반환한다. 이는 ADR 0029의 표현 손실을 가진 ApproximateRatio를 금액으로 바꾸는 것이 아니다. 맨 Money를 받던 checked_add_krw는 제거한다.

## 원화 평가 합과 D-05

KrwValuationComponent의 닫힌 집합은 Unconverted(UnconvertedKrw), Converted(MidConvertedMoney)다. UnconvertedKrw는 원화 정규형 문자열 입력에서만 만들고 Money·Amount로 조립하는 공개 생성자를 두지 않는다. 환산 결과의 Money를 꺼내 이 타입에 직접 재포장할 수 없다. MidConvertedMoney는 FxRate::convert_for_aggregation이 Mid를 검사한 뒤에만 외부에 내준다. Buy 환산 결과를 Converted 분기에 넣으면 컴파일되지 않는다.

KrwValuation은 from_component로 시작하고 같은 타입끼리 checked_add한다. 합산은 모든 FxRate를 보존하고 객체 전체가 같은 환율만 중복 제거한다. 값·종류·방향·출처·시작·종료 중 하나라도 다르면 별개다. 환율이 하나도 없으면 Exact, 하나라도 있으면 Approximate다. 금액을 꺼내는 공개 경로는 into_parts() -> (Money, Vec<FxRate>, ValuationAccuracy) 하나뿐이며 From·Deref·금액 전용 접근자는 없다. 합산 실패 시 원본 두 값은 변하지 않는다.

서로 다른 환율은 64개까지 허용하고 65번째 고유 환율은 TooManyRates다. 64는 토스의 업무 한도가 아니라 내부 메모리와 중복 비교 작업량을 제한하는 보수적인 자원 상한이다. 한 시점의 한 USD/KRW 환율을 여러 종목에 재사용하는 일반 포트폴리오는 보통 하나만 필요하고 64는 다양한 과거 시점의 합산에도 여유를 준다. 원화 종목 수는 이 한도를 소모하지 않는다. 상한에서 같은 환율을 다시 쓰는 것은 허용한다. 다른 환율을 버리거나 가장 최근 환율로 대체하지 않는다. 초과 시 처리 UX는 D-05가 정한다.

D-05 완료 기준은 통화별 API 평가액의 정확한 일치와 원화 환산 총액·총자산의 근사 표시다. 어댑터가 받은 원화 평가액을 UnconvertedKrw로 파싱하고 USD 평가액은 MidConvertedMoney로 환산한 뒤 KrwValuation으로 더할 수 있다. 원화만 있으면 Exact이며 외화가 하나라도 들어가면 Approximate라서 D-05가 이 타입을 그대로 표시 모델의 근거로 사용할 수 있다. 표시 모델에는 금액뿐 아니라 환율 목록과 정확도를 전달해야 한다. 현금 대신 매수가능금액을 넣은 '총자산'은 ACC-6의 별도 근사 요인이므로 D-05가 원화만 있는 경우에도 근사로 표시해야 한다. 이 타입의 Exact는 통화 환산으로 인한 근사가 없다는 뜻이며 총자산의 경제적 정확성 인증이 아니다.

소수 15자리 환율과 소수 15자리 금액처럼 정확히 표현할 수 없는 곱은 PrecisionLoss다. 이 오류를 그대로 전파하면 해당 총액 계산도 실패한다. D-05가 그 종목을 평가 불가로 표시하고 나머지를 합산할지, 총액 전체를 평가 불가로 둘지 정해야 하며 이번 작업은 오류를 삼키거나 0으로 대체하지 않는다. 최소 양수 1e-28 KRW/USD도 수학적으로는 허용한다. 실제 환율 크기의 상식 범위와 응답의 진실성 검사는 T-08 어댑터 책임이다.

## 역방향 결정

이번에는 USD→KRW만 구현한다. R-11의 평가·비중, D-05·D-31, D-32의 달러 주문 고액 판정은 모두 곱셈이다. design 6.3·D-25는 미국 매수에 USD 매수가능금액만 사용하고 달러가 없으면 환전 안내를 하므로 KRW 예산을 USD로 나누는 주문 경로가 필요하지 않다. R-11의 '지금 달러를 사면'은 환율 종류의 사용 설명이지 이번 작업의 환전 실행 요구가 아니다.

향후 원→달러 견적이 필요하면 자릿수와 반올림 방식을 인자로 받는 별도 정책을 먼저 확정해야 한다. 최대 정밀도 근사 몫을 Amount로 재포장하지 않는다. 이번에 반올림 API를 선제 추가하지 않는다.

## 현실적인 정밀도 예산

스펙 예시 1380.5(소수 1자리) × USD 5000.00(2자리)는 소수 3자리이며 계수 13,805×500,000=6,902,500,000으로 96비트 안이다. 보수적으로 환율 소수 8자리와 금액 소수 8자리를 가정해도 합 16자리로 28자리 안이다. 예를 들어 1375.12345678 × 123456789.12345678은 계수 137512345678 × 12345678912345678 = 1697683266224073934965279684(28자리)이고 96비트 최대 79228162514264337593543950335보다 작아 정확하다.

그러나 토스는 decimal 문자열 maxLength 30만 지정하고 환율 소수 상한을 따로 정하지 않는다. 모든 현실 응답이 8자리 이하라고 보장할 근거는 없다. 소수 자릿수 합뿐 아니라 96비트 계수 한계도 검사해야 한다. 유효 입력 1e-28 USD × 1.1은 소수 29자리를 요구해 PrecisionLoss가 된다. 극단값도 묵시적으로 반올림하지 않는다.

## 검증과 제한

속성 테스트는 Config::with_cases(4096)이며 구현과 독립인 i128 정수 계수 곱과 스케일 합을 기대값으로 사용한다. 양수 환율, 부호 있는 금액, 두 종류, 시각 보존을 검사한다. R-11 예제 7, 잘못된 통화, Mid 정책, 오버플로·표현 손실을 고정 예제로 검사한다. 기존 산술 모듈의 96비트 전체 범위 독립 기준 검증은 재사용하며 이번 속성 생성기는 실제 계산의 정확 성공 사례에 집중한다.

적대적 리뷰 재현은 different_mid_rates_are_all_preserved_and_duplicates_are_deduplicated, sixty_four_distinct_rates_are_bounded_without_losing_context, validity_is_start_inclusive_end_exclusive_and_history_remains_available, conservative_selection_keeps_the_selected_rate_and_rejects_other_pairs에 고정했다. 환율 목록에서 두 번째 값을 버리기, Exact/Approximate 뒤집기, Buy를 Mid 래퍼로 허용하기, 시작 제외, 종료 포함의 손 변형 5개는 모두 컴파일된 뒤 해당 단언에서 실패했다. 원본 복구 뒤 D-04 테스트 11개가 통과했다. 외부 crate에서 Money 정규형 파싱 성공과 재포장 두 경로·Buy 합산·Money→UnconvertedKrw·Mid 래퍼 위조·금액 필드 직접 접근의 컴파일 실패도 확인했다. 이 실험은 영속 compile-fail 하네스를 추가한 것은 아니다.

프로젝트 설정의 hooks=true와 Git 훅 설치는 확인한다. 현재 T3 세션의 프로젝트 신뢰와 Codex 훅 신뢰가 각각 적용되었는지는 확인할 증거가 없으므로 적용 완료로 주장하지 않는다. 직접 실행한 시작 검사와 커밋 훅의 전체 검사 결과를 증거로 남긴다. 적대적 리뷰·비판·최종 평가는 메인 절차다.
