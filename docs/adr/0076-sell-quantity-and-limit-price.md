# ADR 0076: D-26 매도 수량과 지정가

- 상태: 확정 (2026-11-15)

- 기준 커밋: 54668ffd6a0cd0e49338067a4b9ef54ded752fc5

## 근거와 결정

설계 6.3의 근거 문장은 “보유 수량과 매도가능수량 안에서 투자 성향 규칙으로 정한다”, “기본은 지정가다. 지정가는 코드가 최신 호가와 호가 단위로 정한다. 시장가는 고급 설정에서 켜야 한다”, “수량과 가격 규칙의 세부 값(한 번에 사는 비중, 매도 비율, 지정가 산정식)은 구현 단계 ADR로 정한다”다. 마지막 문장에 따라 다음 값은 확정 정책이 아닌 구현 초안이다. 설계 문서에 없는 매도 비율이나 지정가 숫자를 원문 예제라고 주장하지 않는다.

수량 목표는 코드가 정한 SellTarget::All 또는 Fraction(ProfileWeight)이다. 비율은 전체 보유 수량에 곱한 뒤 보유 수량·매도가능수량의 최솟값으로 제한한다. 정수 주문은 내림, 미국 소수 시장가 매도는 소수 6자리 내림이다. 국내 소수 보유는 D-34와 일관되게 닫힌 오류다. 결과 0은 BelowMinimumUnit이고, 정확 곱의 표현 손실은 Arithmetic이다. 금액 주문으로 자동 전환하지 않는다. 목표 선택과 투자 성향별 비율 대응은 호출 측 코드가 하며 AI가 수량을 정하지 않는다.

지정가는 매수의 최우선 매도호가, 매도의 최우선 매수호가를 기준으로 추가 여유 0틱이다. 기준 호가가 단위에 맞으면 그대로 쓴다. 맞지 않으면 매수는 올림, 매도는 내림으로 체결 가능한 가장 가까운 단위를 선택한다. 이는 사용자에게 추가 틱 여유를 부과하지 않는 최소 조정이다. 체결 가능성을 유지하는 올림·내림과 원래 불일치 호가보다 유리한 가격은 동시에 보장할 수 없으므로, 불리함이 전혀 없다고 주장하지 않는다. 이 방향은 문서에 이미 정해진 사실이 아니라 설계 6.3이 위임한 산정식 초안이다. 최종 가격을 PriceLimits로 대조하며 상·하한 밖이면 자르거나 대체하지 않고 OutsideLimits다. 반대쪽 최우선 호가가 없거나 그 잔량이 0이면 계산하지 않으며 다음 단계를 임의로 고르지 않는다.

ADR 0067의 책임에 따라 서버 호가 시각·수집 시각과 가격 제한 시각·수집 시각 모두 계산 시점보다 미래가 아니고 5초 이하일 때만 받는다. 5초는 나노초까지 포함하며 그보다 1ns 오래되면 거부한다. 서버 시각 없음은 UnknownTimestamp다. 원시 시각의 최대 2초 미래 허용과 별개로 계산 시점보다 미래인 사실은 보수적으로 거부한다. 호가 시각 10, 제한 시각 9, 수집 시각 11이면 계산 시각 14까지 허용한다. 5초는 사용자 확인 대상 초안이며 새 설정 화면은 만들지 않는다.

표는 D-35의 봉인된 TickTable을 사용하고 Option::None을 TickUnavailable로 표현한다. 종목은 호가·표·가격 제한의 QuoteInstrument 전체를 대조한다. 국내 표 적용이 불명확하여 TickTable을 만들 수 없으면 호출 측이 None을 전달하며 임의 단위를 만들지 않는다.

## 결합과 정책

SellQuantityPlan의 필드는 비공개이며 계좌·주문 종목·매도 수량을 결합한다. 생성 시 SellableQuantityFact의 계좌와 종목을 보유 종목·요청 계좌와 대조한다. Holding 자체에는 계좌 필드가 없으므로 주어진 Holding이 해당 계좌에서 왔다는 진위는 어댑터·O-01의 책임이다. 현재 ResolvedOrderInstrument에는 Symbol·Market만 있으므로 매도 수량과 가격을 합칠 때 이 둘을 대조한다. 시세 사실 간에는 내부 종목 ID까지 대조한다.

CalculatedLimitPrice는 종목·방향·가격을 봉인한다. bind_sell_limit는 Sell 방향·같은 주문 종목·정수 수량만 결합한다. MarketOrderPolicy는 실제 D-12 Policy와 SavedMarketOrderSetting을 비공개 필드로 소유하는 스냅샷이다. default_market_order_policy(Policy)는 항상 꺼짐이다. restore_market_order_policy(SavedMarketOrderPolicy)는 실제 Policy와 타입 있는 저장 설정을 함께 복원한다. 공개 Enabled 정책 변형은 제거했다. bind_sell_market는 스냅샷의 저장 상태를 exhaustive match로 검사하며 켜짐일 때만 해당 Policy를 봉인된 결과 내부에 옮긴다. 기존 Policy impl은 변경하지 않았다. 이 복원 함수는 HoursPermission/InvestorProfile의 저장 상태 복원과 같은 신뢰 경계이며, 임의 입력이 진짜 저장 자료인지 또는 네이티브 확인을 거쳤는지는 인증하지 않는다. A-17/O-14/S-10은 승인된 설정 저장소에서 읽은 값만 공급한다. 불린 인자와 새 사용자 절차는 없다.

CalculatedSellOrder는 계좌·종목·매도 방향의 수량과 Limit(Price)/Market를 한 값으로 묶는다. sell_order_into_intent는 이 값을 소비하고 ClientOrderId만 별도로 받아 D-13 OrderIntent를 만든다. 호출자가 계좌·종목·방향·수량·가격을 재조립하지 않는다. 시장가 결과에는 허용 검사한 동일 Policy가 비공개 내부 변형으로 보존되며 읽기 전용 접근자만 있다. 변환에 정책이나 설정을 바꿔 넣는 인자는 없으므로 계산 시의 허용 조건이 유지된다. 지정가 미국 가격은 D-35 결과를 UsOrderPrice로 검증해 변환한다. 비판자의 8,192경우와 경계 3개 통과를 가격 정밀도 결함으로 재분류하지 않고 고정 회귀를 추가했다. 생성자·필드는 봉인했고 승인·전송 권한은 없다. 매도 방향은 타입 이름과 생성 함수로 고정한다. 계산 결과와 시장가 변형의 Debug는 금액 원문을 내보내지 않는다. 오류는 입력값을 담지 않는 닫힌 열거형이다. 기존 OrderIntent 생성자를 바꾸거나 모든 직접 생성 경로의 정책 검증까지 보장하지 않는다.

## 예제 대조

| 출처 / 입력 | 독립 기대값 | 고정 테스트 |
|---|---|---|
| 설계 6.3 상한, 초안 예제: 보유 17, 매도가능 11, 전량 | 11주 | sell_examples_and_identity |
| 초안 예제: 보유 7, 매도가능 19, 전량 | 7주 | sell_target_examples |
| 초안 예제: 보유 17, 매도가능 11, 비율 50% | floor(min(8.5,17,11)) = 8주 | sell_target_examples |
| 미국 소수: 보유 1.23456789, 매도가능 2.4 | 1.234567주 (시장가만) | sell_zero_fraction_and_precision |
| ADR 0067의 49,990원 경계 예제: 매도 기준 | 49,950원 | limit_examples_side_rounding_and_boundary |
| 국내 매수 기준 50,001원 | 50,100원 | limit_examples_side_rounding_and_boundary |
| ADR 0067의 0.99995달러 경계 예제: 매수 기준 | 1.00달러 | limit_examples_side_rounding_and_boundary |
| 미국 매도 기준 0.99985달러 | 0.9998달러 | limit_examples_side_rounding_and_boundary |

R-01 173행은 국내 정수·호가 단위, 미국 1달러 미만 4자리/이상 2자리 조합을 설명한다. 설계 6.3과 R-11에는 이 작업의 매도 비율·지정가 산정식 수치 예제가 없다. 없는 예제를 발명해 원문으로 귀속하지 않는다.

## 검증

초기 테스트 우선 실패는 red.log·limit-red.log에, 이번 정책 API 수정의 실패는 revision-red.log에 보존한다. 최종 대상 테스트는 27개다. 기존 정수 수량·국내 가격 속성에 더해 변하는 가격 제한·미국 소수 수량·미국 가격의 독립 정수 대조를 각각 4,096회 실행한다. 미국 가격은 양 방향 8,192개 결과이며 가격 자릿수 변환도 대조한다. 348개 mantissa·scale·정밀도 조합과 모든 국내 가격대·미국 1달러·가격 제한 양 끝을 고정한다. 계산·정책·테스트 파일 7개의 줄·영역 커버리지 100%와 domain all-targets clippy, source-check·raw-types를 확인한다. 전체 검사는 정상 커밋 훅의 just check로 수행하며 결과는 target/D-26/revision-commit.log에 보존한다. 손 변형은 자동 변이 도구 대신 최종 코드의 분기·경계·연산자를 바꿔 실제 실행한다.

새 외부 의존성·검사 끄기 없음. 이전 D-26에서 추가한 decimal-rounding 항목 하나만 제거했다. mantissa의 각 버린 십진 자릿수를 정수 10으로 나누고 목표 scale로 재조립하므로 새 반올림 예외나 raw-type 허용 목록은 없다. 원시 타입은 함수 시그니처·필드에 추가하지 않았으며 지역 추론값으로만 정수 내림을 수행한다. 기존 master 테스트/impl·tasks.md·progress.md는 수정하지 않는다. 프로젝트 신뢰와 세션 훅 신뢰는 각각 확인 불가이며 홈 설정·자격증명은 읽거나 수정하지 않는다. Git 훅 설치와 정상 커밋 훅 결과를 별도 근거로 사용한다.

## 인계

- A-14: SellTarget을 신호·정책 코드에서 선택하고 기본 정수 경로로 계산한다. 시장가로 자동 전환하지 않는다. OrderIntent는 sell_order_into_intent에 CalculatedSellOrder와 ClientOrderId만 넘겨 만든다.
- O-01/O-04: 매도가능수량·보유의 계좌 진위와 최신성, 장 시간·소수 매도 접수 시간, 정책 최신값을 승인 직전에 재검증한다. 우회한 OrderIntent 직접 생성 경로에서도 시장가 정책을 검증한다. 주문 방향·종목과 호가 결합을 유지한다.
- D-25/O-01: 매수 지정가는 check_buy_plan_order_price(...).into_valid()의 PriceBoundBuyPlan을 실제 주문 경로에서 요구한다. 산정가보다 높은 올림 결과는 수량을 다시 계산하며 시장가·금액 매수로 자동 전환하지 않는다. 이 작업에는 별도의 매수 주문 결합 경로를 추가하지 않았다.
- A-17/O-14: SavedMarketOrderPolicy 복원 경계에 실제 위험 설정 확인·저장소에서 복원한 Policy와 SavedMarketOrderSetting을 공급한다. D-12 Policy 구조는 이 작업에서 수정하지 않았다.
- E-92/O-15: 비율 선택·0틱 산정·5초 신선도 초안과 국내 호가 표를 실측·평가한다. 실제 계좌·키·API 호출은 사용자 몫이다.

## 메인 평가에 따른 수정

반올림 예외는 제거했고 비판자의 floor.rs는 읽기만 했다. scale 0..28·mantissa 0/1/9/123456789/u64::MAX/Decimal 최대값과 정밀도 0/6의 348조합을 실제 계산 경로로 독립 정수 나눗셈 기대값과 대조한다. 미국 소수 6자리 수량의 독립 microshare 정수 대조는 4,096회다. 미국 가격 역시 4,096회 양 방향을 독립 정수 단위로 계산하여 1달러 가격대 전환과 UsOrderPrice 정밀도를 검증한다. 변하는 국내 하한·상한과 범위 밖 결과는 4,096회 독립 정수 호가 단위 계산과 대조한다. 국내 일반 표의 모든 여섯 가격대 경계 양쪽·정확 경계와 미국 1달러 경계를 고정했다. 상하한 양 끝 포함도 고정했다.

방향·국내 여부·시장 결합·설정 상태의 enum 판정은 exhaustive match로 바꿨다. 서로 다른 종목·계좌의 식별 값 대조는 그대로 유지한다. PriceLimitsError는 LimitPriceError::Limits 원래 payload로 보존하며 OutsideLimits는 실제 BelowLower/AboveUpper에만 쓴다. 주문 값·주문 의도 오류도 원래 닫힌 payload를 보존한다. 미래 시각 거부와 O-01/O-04 보유 계좌 진위·사실 신선도 인계는 유지한다.

기존 OrderIntent::new 직접 생성 경로는 그대로다. 새 D-26 시장가 API는 Policy 없는 켜짐 열거형을 받지 않으며 봉인된 스냅샷의 허용 검사를 요구한다. 저장 경계의 진위 확인과 승인 직전 최신 정책 재검증은 여전히 O-01/O-04/A-17/O-14 책임이다. AI와 주문 전송 포트 연결은 없다. tasks.md·progress.md·기존 master 테스트/impl은 변경하지 않는다.

## 손 변형 실제 실행

| 번호 | 파일 / 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | sell_quantity.rs: 계좌 대조 반전 | limit_price_revision::tests::identical_symbol_in_another_market_cannot_bind | 검출 |
| 2 | sell_quantity.rs: 종목 대조 반전 | limit_price_revision::tests::mantissa_floor_covers_348_scale_and_precision_cases | 검출 |
| 3 | sell_quantity.rs: 국내 소수 거부 반전 | limit_price_revision::tests::identical_symbol_in_another_market_cannot_bind | 검출 |
| 4 | sell_quantity.rs: 전량을 1주로 제한 | limit_price_revision::tests::mantissa_floor_covers_348_scale_and_precision_cases | 검출 |
| 5 | sell_quantity.rs: 비율을 전량으로 확대 | sell_quantity_review::tests::fractional_target_below_one_has_no_order | 검출 |
| 6 | sell_quantity.rs: 목표와 보유 최소를 최대로 | sell_quantity_review::tests::fractional_target_below_one_has_no_order | 검출 |
| 7 | sell_quantity.rs: 매도가능 상한을 하한으로 | limit_price_revision::tests::consuming_conversion_preserves_all_fields_and_us_precision | 검출 |
| 8 | sell_quantity.rs: 미국 소수 6자리에서 5자리 | limit_price_revision::tests::market_conversion_preserves_policy_checked_quantity | 검출 |
| 9 | sell_quantity.rs: 목표 scale 최소를 최대로 | limit_price_revision::tests::mantissa_floor_covers_348_scale_and_precision_cases | 검출 |
| 10 | sell_quantity.rs: 버린 자릿수 10 대신 100 나눗셈 | limit_price_revision::tests::mantissa_floor_covers_348_scale_and_precision_cases | 검출 |
| 11 | sell_quantity.rs: 0수량 사유 교체 | limit_price_revision::tests::mantissa_floor_covers_348_scale_and_precision_cases | 검출 |
| 12 | limit_price.rs: 신선도 5초에서 6초 | limit_price_review::tests::limit_rejects_missing_stale_and_mismatched_facts | 검출 |
| 13 | limit_price.rs: 나노초 경계 반전 | limit_price_review::tests::limit_ranges_and_tick_errors_are_closed | 검출 |
| 14 | limit_price.rs: 매수 기준을 매수호가로 | limit_price_revision::tests::inclusive_price_limits_are_preserved | 검출 |
| 15 | limit_price.rs: 매도 기준을 매도호가로 | limit_price_revision::tests::inclusive_price_limits_are_preserved | 검출 |
| 16 | limit_price.rs: 매수 올림을 내림으로 | limit_price_review::tests::limit_matches_independent_tick_reference | 검출 |
| 17 | limit_price.rs: 매도 내림을 올림으로 | limit_price_revision::tests::consuming_conversion_preserves_all_fields_and_us_precision | 검출 |
| 18 | limit_price.rs: 잔량 0 거부 반전 | limit_price_review::tests::limit_examples_side_rounding_and_boundary | 검출 |
| 19 | limit_price.rs: 미제공 시각 사유 교체 | limit_price_review::tests::limit_rejects_missing_stale_and_mismatched_facts | 검출 |
| 20 | limit_price.rs: 호가 표 종목 대조 반전 | limit_price_revision::tests::every_domestic_band_and_us_dollar_boundary_is_fixed | 검출 |
| 21 | limit_price_order.rs: 주문 심볼 대조 반전 | limit_price_review::tests::fractional_limit_and_foreign_instrument_cannot_bind | 검출 |
| 22 | limit_price_order.rs: 소수 지정가 거부 반전 | limit_price_review::tests::fractional_limit_and_foreign_instrument_cannot_bind | 검출 |
| 23 | limit_price_policy.rs: 꺼진 저장 정책 시장가 허용 | limit_price_review::tests::sell_order_binds_quantity_price_and_policy | 검출 |
| 24 | limit_price_policy.rs: 정책 기본 꺼짐을 켜짐으로 | limit_price_review::tests::sell_order_binds_quantity_price_and_policy | 검출 |
| 25 | limit_price_policy.rs: 저장된 켜짐 복원 누락 | limit_price_revision::tests::market_conversion_preserves_policy_checked_quantity | 검출 |
| 26 | limit_price_order.rs: 주문 변환 계좌 교체 | limit_price_revision::tests::consuming_conversion_preserves_all_fields_and_us_precision | 검출 |
| 27 | limit_price_order.rs: 국내 시장가 변환 방향 교체 | limit_price_revision::tests::market_conversion_preserves_policy_checked_quantity | 검출 |
| 28 | limit_price_order.rs: 미국 지정가 변환 방향 교체 | limit_price_revision::tests::consuming_conversion_preserves_all_fields_and_us_precision | 검출 |
| 29 | limit_price.rs: 가격 제한 오류 payload 축약 | limit_price::tests::price_limit_failure_keeps_original_reason | 검출 |

이번 수정의 최종 코드로 mktemp -d 복사본에서 29개를 각각 실제 실행했다. 29개 모두 컴파일되었고 29개 모두 검출, 생존 0·컴파일 오류 0이다. 원본에 변형을 적용하지 않았다. 실행 명령은 cargo nextest run --locked -p domain -E 대상 필터이며 target/D-26/run-revision-mutations.py·mutation-01.log부터 mutation-29.log·mutations.json에 재현 자료를 남겼다.
