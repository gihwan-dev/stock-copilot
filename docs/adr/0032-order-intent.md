# ADR 0032: D-13 닫힌 주문 의도와 조회로 확정한 시장

상태: 확정 (2026-11-14). 최초 기준 cdb712fc4fab831c27fc81549372cc706acc032b, 수정 기준 5f688a0f9f2e0bc2e10516c27712524a8b687884.

## 허용 조합과 차단 방식

Q는 `docs/toss-openapi-1.2.28.json`의 `OrderCreateRequest.oneOf[0]`(OrderCreateQuantityBased), A는 oneOf[1](OrderCreateAmountBased), C는 `ConditionalOrderCreateRequest`다. 금액 지정에는 수량 필드가 없다. 정수·소수는 정규화한 수학적 값으로 판단한다. 아래 테스트 약호는 바로 뒤 표의 실제 테스트 이름을 뜻한다.

| 시장 | 방향 | 유형 / 유효조건 | 수량 정수 | 수량 소수 | 금액 (수량 없음) | 근거 |
|---|---|---|---|---|---|---|
| KR | 매수 | LIMIT / DAY | 허용: 생성자 QB, 조합 I | 생성자로 막음(QB) | 타입으로 막음(변형 없음) | Q.quantity 양의 정수, Q.price 필수, A US MARKET 전용 |
| KR | 매도 | LIMIT / DAY | 허용: 생성자 QB, 조합 I | 생성자로 막음(QB) | 타입으로 막음(변형 없음) | 위와 같음 |
| KR | 매수 | MARKET / DAY | 허용: 생성자 QB, 조합 I | 생성자로 막음(QB) | 타입으로 막음(변형 없음) | Q.quantity, Q.price 시장가 가격 전달 불가, A |
| KR | 매도 | MARKET / DAY | 허용: 생성자 QB, 조합 I | 생성자로 막음(QB) | 타입으로 막음(변형 없음) | 위와 같음 |
| US | 매수 | LIMIT / DAY | 허용: 생성자 QB, 조합 I | 생성자로 막음(QB) | 타입으로 막음(변형 없음) | Q.quantity 소수는 MARKET SELL만, A.orderType MARKET만 |
| US | 매도 | LIMIT / DAY | 허용: 생성자 QB, 조합 I | 생성자로 막음(QB) | 타입으로 막음(변형 없음) | 위와 같음 |
| US | 매수 | MARKET / DAY | 허용: 생성자 QB, 조합 I | 생성자로 막음(QB) | 허용: 생성자 AB, 조합 AI | Q.quantity 소수 매수는 orderAmount 사용, A.side BUY/SELL |
| US | 매도 | MARKET / DAY | 허용: 생성자 QB, 조합 I | 허용(6자리): 생성자 QB, 조합 F | 스펙 허용·제품 제외: 타입으로 막음(변형 없음) | Q.quantity MARKET SELL만 소수 6자리. A는 SELL도 허용하지만 design 6.3 매도 수량, 6.5 매도가능수량 한도, D-26의 수량 상한을 제품에서 보장 |

| 약호 | 테스트 이름 |
|---|---|
| QB | `quantity_boundaries_and_wire_length`, `quantity_matches_independent_integer_table` |
| I | `every_allowed_day_cell_and_all_intent_fields_are_readable` |
| F | `fractional_us_market_sell_is_allowed` |
| AB | `amount_requires_positive_usd_and_canonical_wire_value`, `amount_stores_cents_and_exposes_canonical_wire_strings`, `amount_precision_matches_independent_cent_table` |
| AI | `amount_order_is_us_market_buy_only` |

수량 지정이 기본이며 금액 지정으로 자동 전환하지 않는다. design 6.3은 매수 금액을 정해 수량으로 바꾸고, 매도는 보유 수량과 매도가능수량 안에서 정한다. 금액 매도는 체결가로 수량이 정해져 이 상한을 정확히 검증할 수 없으므로 제품에서 제외한다. 지정가 기본·시장가 설정 허용 여부는 D-26/O-01 정책이다.

| 추가 조합 (양 방향, 양 지정 방식, 정수·소수 전체) | 판단과 차단 방식 | 근거 |
|---|---|---|
| KR LIMIT/MARKET + OPG | 수량 정수는 스펙 허용이나 제품 제외. 타입으로 막음(변형 없음). 나머지는 스펙 금지이며 변형 없음 | Q.timeInForce, design 6.5 앱 DAY만 |
| US LIMIT + CLS | 수량 정수는 스펙 허용이나 제품 제외. 타입으로 막음(변형 없음). 나머지는 스펙 금지이며 변형 없음 | Q.timeInForce, design 6.5 |
| KR CLS, US OPG, MARKET CLS | 스펙 금지. 타입으로 막음(변형 없음) | Q.timeInForce 지원 조합 한정 |
| SINGLE / OCO / OTO 조건부 | 제품 제외. 상세 조합은 미해결로 금지. 타입으로 막음(변형 없음) | C.type, C.orderType(OCO/OTO LIMIT만), design 6.5 |
| Unknown 시장 | 생성자로 막음(`master_lookup_market_must_match_order_terms_and_unknown_is_rejected`) | ADR 0022 Unknown은 조회 보존용 |
| 문서에 없는 유형·가격 유형 | 근거 없음으로 금지. 타입으로 막음(변형 없음) | 보수적 주문 입구 |

## 종목과 시장

OrderIntent는 맨 Symbol을 받지 않고 비공개 필드의 ResolvedOrderInstrument를 받는다. 공개 생성 경로는 `ResolvedOrderInstrument::from_master_lookup_result(Symbol, Market)`이며 Unknown은 거부한다. 호출자가 심볼 모양이나 주문 조건으로 시장을 추측해 붙이면 안 된다. T-09 종목 마스터 조회 결과에서 이 값을 구성하고, O-01이 주문 직전 최신 종목 조회와 대조해야 한다. domain은 주어진 조회 결과가 사실인지 인증할 수 없다. 조회 결과라는 이름은 이 책임을 드러내며 IO 증명 토큰을 대신하지 않는다.

`OrderIntent::new(AccountId, ResolvedOrderInstrument, ClientOrderId, OrderTerms)`는 Result를 반환하고 종목 시장과 조건 시장이 다르면 MarketMismatch다. AAPL/US + KrLimit(70000 KRW), 005930/KR + UsLimit(70000 USD), Unknown 시장 생성 거부를 `master_lookup_market_must_match_order_terms_and_unknown_is_rejected`로 고정한다. 같은 시장의 전체 허용 조합도 I/AI에서 생성 성공을 확인한다. 토스 주문에는 시장 필드가 없으므로 API 조립이 다른 시장으로 재해석하게 둘 수 없다.

## 닫힌 주문 조건과 전송 표현

OrderTerms의 변형은 KrLimit, KrMarket, UsLimit, UsMarketBuy, UsMarketSell, UsMarketAmountBuy다. 금액 매수에는 방향 필드가 없고 side()는 Buy다. 지정가에는 시장별 가격이 필수이며 시장가에는 가격이 없고 금액에는 수량이 없다. 상태 플래그나 임의 유형 문자열을 받지 않는다.

WholeOrderQuantity는 양수 정수, SellOrderQuantity는 양수·소수 6자리 이하. KrOrderPrice는 KRW 양수 정수, UsOrderPrice는 USD 양수이고 $1 미만 4자리/$1 이상 2자리다. Quantity와 Price의 checked 생성자를 재사용해 정규화한 값을 저장한다. 원래 스케일을 보관하지 않는다. USD 가격의 1달러 경계는 cmp 결과의 Ordering match로 표현해 기존의 `<`→`<=` 동등 변이를 없앤다.

다섯 값 타입은 각각 `wire_value()`로 봉인된 OrderWireValue를 반환하며 `as_str()`로 정규 문자열을 읽는다. 어댑터가 자릿수를 정하거나 반올림·정규화하지 않는다. 정수 수량 `1.000000`→`1`, 소수 매도 `0.0000010`→`0.000001`, 국내 가격 `70000.000`→`70000`을 저장 스케일과 실제 문자열로 검사한다. `quantities_store_normalized_values_and_expose_canonical_wire_strings`, `prices_store_normalized_values_and_expose_canonical_wire_strings`가 재현 테스트다. 사후 대조 키도 이 정규 문자열을 사용한다.

UsdOrderAmount는 USD 양수이며 정규화 후 소수 2자리(센트)까지만 허용한다. 스펙이 정한 상한이 아니라 보수적 제품 정책이다. 실제 최소 주문 금액과 자릿수는 토스 연동 테스트에서 실측한다. 임의의 $1 최소 금액은 넣지 않는다. Decimal의 양수 계수는 최대 29자리, scale은 최대 28이므로 정규 문자열은 최대 30자다. `thirty_character_quantity_is_the_actual_wire_value`와 AB에서 실제 전송 문자열을 검사한다.

## 후속 작업과 승인 경계

- D-25는 수량 계산, D-26은 최신 호가·실제 호가 단위·매도 수량 상한과 시장가 정책을 맡는다.
- O-01은 종목 마스터의 사실성, 장 운영·잔고, 금액 주문 및 소수 수량 매도의 정규장 시작부터 종료 1시간 전 접수 제한을 검사한다. 메인이 tasks.md O-01 행에 시간 제약을 반영한다.
- 수량과 가격이 각각 유효해도 곱이 넘칠 수 있다(예: Decimal::MAX 수량과 가격). D-32는 Price::checked_mul의 오류를 주문 불가로 처리해야 하며, 오류를 0·최댓값 등 다른 금액으로 바꾸면 안 된다.
- ClientOrderId는 호출자가 넣는다. D-14는 제안 생성 시 키를 고정하고 O-18은 제안 ID에서 결정적으로 만든다. 같은 의도에 다른 키를 붙여 중복 주문을 만들지 않아야 한다. 소비 기록·재시도는 O-18/O-19다.
- 근거 신호, 만료, 검증 보고서와 승인 상태는 D-14 Proposal에 둔다. 고액 판정은 D-32, 고액 동의는 승인 경로다. OrderIntent는 승인 토큰이 아니며 전송 포트가 없다.
- AccountId Debug는 ADR 0022의 숫자 키 표현을 유지한다. 계좌번호는 받지 않는다. 오류에 입력값을 싣지 않는다.

## 검증과 미해결

속성 테스트는 수량·가격·금액 각각 Config::with_cases(4096)이며 독립 정수 계수·나머지 기준과 대조한다. 영속 compile-fail 하네스는 ADR 0022/0029의 제약으로 추가하지 않는다. 외부 crate 실험으로 맨 Symbol 전달, 금액 매도 변형·방향 주입, 필드 위조와 비공개 생성자 접근을 확인한다. 시장 불일치는 외부에서도 같은 공개 Result 생성 경로를 사용하며 재현 테스트가 MarketMismatch를 확인한다.

손 변형은 시장 대조 제거, 수량·가격·금액의 정규화 저장 제거, 금액 자릿수 2→3을 주입하고 재현 테스트가 실패하는지 확인한 뒤 복원한다. 자동 변이 검사는 마지막에 한 번 실행한다. 결과와 최종 SHA는 구현 보고에 남긴다. 새 의존성·검사 끄기·기존 master 테스트 수정·실주문·키·계좌 접근은 없다. 프로젝트 설정 hooks=true와 Git 훅 설치는 확인했지만 현재 세션의 프로젝트 신뢰·훅 신뢰 적용 자체는 확정할 증거가 없어 직접 검사 및 Git 훅 결과를 사용한다.
