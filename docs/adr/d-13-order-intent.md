# ADR 초안 D-13: 닫힌 주문 의도

상태: 초안. 기준 cdb712fc4fab831c27fc81549372cc706acc032b.

## 허용 조합과 근거

Q는 `OrderCreateRequest.oneOf[0]`(OrderCreateQuantityBased), A는 oneOf[1](OrderCreateAmountBased), C는 `ConditionalOrderCreateRequest`를 가리킨다. 모두 저장소의 toss-openapi-1.2.28.json 사본이다. 아래 매수·매도는 각각 독립 칸이며 정수·소수는 수학적 값 기준이다. 금액 지정에는 수량 자체가 없으므로 수량 형태 두 칸 모두 해당 없음이다.

| 시장 | 방향 | 유형 / 유효조건 | 수량 정수 | 수량 소수 | 금액 (수량 없음) | 근거 |
|---|---|---|---|---|---|---|
| KR | 매수 | LIMIT / DAY | 허용 | 스펙 금지 | 스펙 금지 | Q.quantity 기본 양의 정수, Q.price 필수, A US MARKET 전용 |
| KR | 매도 | LIMIT / DAY | 허용 | 스펙 금지 | 스펙 금지 | 위와 같음 |
| KR | 매수 | MARKET / DAY | 허용 | 스펙 금지 | 스펙 금지 | Q.quantity, Q.price 시장가 가격 전달 불가, A |
| KR | 매도 | MARKET / DAY | 허용 | 스펙 금지 | 스펙 금지 | 위와 같음 |
| US | 매수 | LIMIT / DAY | 허용 | 스펙 금지 | 스펙 금지 | Q.quantity 소수는 MARKET SELL만, A.orderType MARKET만 |
| US | 매도 | LIMIT / DAY | 허용 | 스펙 금지 | 스펙 금지 | 위와 같음 |
| US | 매수 | MARKET / DAY | 허용 | 스펙 금지 | 허용 | Q.quantity 소수 매수는 orderAmount 사용, A.side BUY/SELL |
| US | 매도 | MARKET / DAY | 허용 | 허용 (6자리) | 허용 | Q.quantity MARKET SELL만 소수 6자리, A.side BUY/SELL |

위 표의 모든 허용은 design 6.3의 수량으로 바꿈·금액 주문은 미국 시장가에서만 문장에 부합한다. 지정가 기본·시장가 설정 허용 여부는 D-26/O-01 정책이며 이 값은 설정 승인 증거가 아니다.

| 추가 조합 (양 방향, 양 지정 방식, 정수·소수 전체) | 판단 | 근거 |
|---|---|---|
| KR LIMIT/MARKET + OPG | 수량 정수는 스펙 허용이나 제품 제외. 나머지는 스펙 금지 | Q.timeInForce, design 6.5 앱 DAY만 |
| US LIMIT + CLS | 수량 정수는 스펙 허용이나 제품 제외. 나머지는 스펙 금지 | Q.timeInForce, design 6.5 |
| KR CLS, US OPG, MARKET CLS | 스펙 금지 | Q.timeInForce 지원 조합 한정 |
| SINGLE / OCO / OTO 조건부 | 제품 제외. 상세 조합은 미해결로 금지 | C.type, C.orderType(OCO/OTO LIMIT만), design 6.5 승인과 전송 한 번에 끝남·DAY만 |
| Unknown 시장 및 문서에 없는 유형·가격 유형 | 근거 없음으로 금지 | ADR 0022 Unknown은 조회 보존용, 보수적 주문 입구 |

## 타입과 경계

OrderTerms의 여섯 변형 KrLimit, KrMarket, UsLimit, UsMarketBuy, UsMarketSell, UsMarketAmount만 제공한다. 방향 공통인 변형은 OrderSide(Buy/Sell)를 갖는다. 시장과 통화는 변형/검증된 가격 타입에서 유도하며 별도 Market 입력이 없다. 시장가에는 가격 필드가 없고 금액에는 수량 필드가 없다. 지정가 가격·수량은 필수다. 모든 필드는 공개 읽기 접근자 또는 열거형 패턴으로 읽는다. 생성자는 값 제약을 검사하고 비공개 필드를 봉인한다.

WholeOrderQuantity는 양수 정수, SellOrderQuantity는 양수·소수 6자리 이하. Quantity의 음수 거부를 재사용한다. 수학적 후행 0은 normalize로 제거하며 정규 전송 문자열은 30자 이하다. Decimal의 양수 계수는 최대 29자리, scale은 최대 28이므로 정규 표기는 정수 최대 29자 또는 소수점 포함 최대 30자다. 따라서 별도의 도달 불가능한 길이 오류 분기를 두지 않고 이 표현 한계와 경계 테스트로 maxLength를 보장한다. API 어댑터는 반드시 정규화해 전송한다. KrOrderPrice는 KRW 양수 정수, UsOrderPrice는 USD 양수이며 $1 미만 4자리/$1 이상 2자리 이하다. 스펙의 묵시적 절삭은 하지 않고 거부한다. 실제 호가 단위와 최신 호가 산정은 D-26, 현재 시세·시간·잔고·시장가 설정·상품/심볼 실제 존재 검사는 O-01이다.

UsdOrderAmount는 USD 양수·정규 문자열 30자 이하. A.orderAmount에 최소 달러 금액·소수 자릿수 상한이 명시되어 있지 않으므로 임의의 $1 최소값 등을 넣지 않는다. API 최소 금액 실측은 미해결이다. 금액 주문 및 소수 수량 매도의 정규장 시작부터 종료 1시간 전 제한은 시각을 받지 않는 D-13이 아니라 O-01에서 반드시 검사한다.

OrderIntent는 AccountId, Symbol, ClientOrderId, OrderTerms를 보유한다. ORD-2와 architecture 4.4의 동일 주문/요약 결합 때문에 키를 필수로 받는다. 발급·제안 ID 기반 결정성·소비/재시도는 O-18/19다. 근거 신호·사용자 요청과의 연결은 D-14 Proposal에 둔다. 만료·검증 보고서·승인 상태·고액 동의 플래그·시각은 넣지 않는다. Debug는 ADR 0022의 AccountId 숫자 키 표현을 그대로 사용하며 계좌번호를 받지 않는다. OrderIntent는 승인 토큰이 아니며 전송 포트가 없다.

## 검증 계획과 미해결

모든 DAY 표 칸을 고정 테스트로 확인한다. 존재하지 않는 변형/필드, 필수 가격 누락, 잘못된 수량 타입, Unknown, 비공개 필드 위조는 외부 crate 컴파일 실험으로 확인한다. ADR 0022/0029처럼 영속 compile-fail 하네스는 없으며 새 의존성·doc 주석을 추가하지 않는다. 속성 테스트는 Config::with_cases(4096)와 독립 정수 계수·자릿수 기준을 쓴다.

새 의존성·검사 끄기·기존 테스트 수정 없음. 프로젝트 설정의 hooks=true는 확인했으나 현재 세션의 프로젝트 신뢰/훅 신뢰 적용은 확인할 증거가 없어 확정하지 않는다. 설치한 Git 훅과 직접 검사 결과를 사용한다.

## 구현 검증 기록

- `quantity_boundaries_and_wire_length`: 0·음수(일반 Quantity 입구), 소수 5/6/7자리, 후행 0, 최대 정수, 30자 수량.
- `quantity_matches_independent_integer_table`: 4096회, 정수 계수와 10의 거듭제곱 나머지로 정수/6자리 조건 대조.
- `prices_require_market_currency_and_exact_api_precision`, `prices_match_independent_integer_tick_table`: 통화 불일치와 KR 정수, US 1달러 경계와 2/4자리. 속성 4096회.
- `amount_requires_positive_usd_and_thirty_character_wire_value`: 0·음수·통화 불일치, 최소 표현 양수 1e-28, 최대 정수, 30자 금액.
- `every_allowed_day_cell_and_all_intent_fields_are_readable`, `fractional_us_market_sell_is_allowed`: 허용 DAY 조합 전부, 식별자·시장·방향 접근자와 계좌 키 Debug.
- 실제 domain을 의존하는 외부 crate 대조군 1개 컴파일 성공. 금지 29개 모두 거부: 소수 수량 타입을 정수 전용 7칸에 전달(E0308), 국내/지정가 금액 6칸(E0559), 지정가 가격 누락(E0063), 시장가 가격·수량금액 동시 지정·시장 주입(E0559), 5개 값 타입 필드 위조(E0423), 의도 필드 위조(E0451), OPG/CLS·조건부 6개 변형(E0599). 추가 조합의 방향·지정 방식·수량 형태 전체에 변형 자체가 없다.
- 기준 검사: 테스트 371개 통과. 구현 후 targeted Clippy 통과, coverage-check 위반 없음(domain 테스트 130개 통과). 영속 compile-fail 테스트는 추가하지 않았다. 실주문·키·계좌 접근 없음.
