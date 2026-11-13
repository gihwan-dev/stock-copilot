# D-08: 거래 행과 결제 멀티셋

- 상태: 초안
- 기준: cdb712fc4fab831c27fc81549372cc706acc032b
- 근거: open-issues I-06, design 5.2, architecture 3·6·8장, R-01 Q5·Q14, R-11 Q1, ADR 0022·0024·0025·0028·0029, 저장소의 OpenAPI 1.2.28 Transaction·TransactionType

## 스키마 대응

국내·해외는 동일한 Transaction 스키마를 쓴다. 모든 필드는 required이며 아래의 선택 값만 null을 허용한다. 거래 ID, 주문 ID, 원거래 ID가 전혀 없다. 종목 ISIN은 거래 식별자가 아니다. 같은 내용의 행이 여러 개 올 수 있으므로 모든 필드의 도메인 값 동등성과 개수를 함께 비교한다. 숫자의 1과 1.0은 같다. 원본 JSON의 철자·스케일은 S-03 어댑터가 별도로 보존한다.

| 토스 필드 | 도메인 표현 | 국내·해외 의미와 null |
|---|---|---|
| transactionType | TransactionKind | 아래 11개 및 Unknown |
| marketCountry | Market | KR/US, 현금·환전은 통화에 따라 시장 결정 |
| currency | Currency | KRW/USD, 모든 금액의 통화 |
| symbol | Option<Symbol> | 주식·옵션 심볼. 채권·현금·확인 불가 주식은 null. 옵션 심볼을 주식 API에 전달하지 않는다 |
| assetClass | Option<AssetClass> | STOCK/OPTION/BOND. 현금은 null. 64바이트 코드로 미래 값도 보존 |
| isinCode | Option<IsinCode> | 거래 당시 주식·채권 ISIN. 옵션·현금은 null. 실제 체크디지트 검증은 하지 않는다 |
| quantity | Option<Quantity> | 매매·증권 입출고. 나머지 null. 옵션 계약 수·채권 개수 |
| filledPrice | Option<Price> | 매매 단가, 알 수 없거나 국내·미국 주식 미결제는 null. 미국 옵션·채권 미결제는 있을 수 있음 |
| amount | Option<Money> | 거래 총액, 증권 입출고 null. 수량×단가로 재계산 금지 |
| commission | Money | 항상 제공, 없으면 0 |
| tax | Money | 항상 제공, 없으면 0. 채권 이자 원화 세금 별도 WITHDRAWAL 가능 |
| netAmount | Option<Money> | 정산 금액, 증권 입출고 null. 미결제는 임시 계산값, 예수금 변화와 다를 수 있음 |
| exchangeRate | Option<Ratio> | 1 USD당 KRW의 양수 비율, USD 결제 완료에서 제공 가능. 없거나 입출고·미결제·KRW이면 null |
| settlementDate | CalendarDate | KST 결제일, 미결제는 예상일. 날짜로 결제 여부 추정 금지 |
| settled | Settlement | false=Pending, true=Settled(transactionTime) |
| transactionTime | Timestamp / Pending에 내재된 null | 결제 처리 시각, 취소 행은 취소 시각. 주문·체결 시각 아님. 미결제 null |
| isCancellation | Cancellation | false=Regular, true=Cancelled |

스펙 금액 문자열 maxLength는 30이며 전송·파싱 어댑터가 검사한다. 도메인은 기존 정확 십진 타입을 쓴다. 금액 음수와 통화 불일치를 거부하고 절삭하지 않는다. Code와 IsinCode는 복제 전 1~64 ASCII 표시 바이트로 제한한다. 이것은 공식 ISIN 문법이 아닌 원문 보존용 내부 상한이다. 새로운 의존성은 없다. FxRate 병렬 작업에 의존하지 않도록 환율은 단위가 고정된 Ratio로 보존하며 환산은 하지 않는다.

| 외부 코드 | TransactionKind |
|---|---|
| BUY | Buy |
| SELL | Sell |
| SECURITIES_TRANSFER_IN | SecuritiesTransferIn |
| SECURITIES_TRANSFER_OUT | SecuritiesTransferOut |
| DEPOSIT | Deposit |
| WITHDRAWAL | Withdrawal |
| EXCHANGE_IN | ExchangeIn |
| EXCHANGE_OUT | ExchangeOut |
| DIVIDEND | Dividend |
| INTEREST | Interest |
| FEE | Fee |
| 그 밖의 값 | Unknown(UnknownTransactionCode), 원래 코드 보존 |

UnknownTransactionCode는 비공개 필드이며 생성자가 없다. TransactionKind::new만 모르는 코드 분기에서 만든다. 알려진 코드를 Unknown에 넣는 공개 경로가 없다(ADR 0022 방식).

## 대조 규칙

호출자는 동일 계좌·필터의 모든 페이지가 성공한 완전한 응답과 결제일 창을 넘긴다. 페이지 중간 결과를 대조하면 안 된다. 계좌·필터 소유권과 하루 전체 조회 스케줄은 A-07·T-06의 책임이다. 창은 닫힌 날짜 구간이며 일반 조회는 최근 14일, 전체 검사는 전체 범위다. 응답의 창 밖 행은 오류이며 저장 행의 창 밖 기록은 그대로 둔다.

| 상황 | 결과 |
|---|---|
| 활성 결제 행 n개, 새 응답 m개 | min(n,m) Unchanged, 부족분 Added, 초과분 Disappeared |
| 사라짐 | 원 행·first_seen 유지, disappeared_at에 동기화 Timestamp 기록 |
| 이미 사라진 행 | 그대로 보존, 사라진 시각 덮어쓰지 않음 |
| 같은 행 재등장 | 과거 사라진 행은 보존하고 새 관측 행 추가 |
| 미결제 응답 | 결제 이력에 넣지 않음. 창 안 임시 집합 전체 교체, 변화 시 PendingReplaced(before, after) |
| 국내 미결제 1행→결제 여러 행 | 임시 집합 제거 및 새 결제 행만 추가. 추정 연결과 임시 금액 계상 없음 |
| 국내 취소 | 원 행은 목록에서 제외되어 Disappeared, 원래 종류를 유지하는 취소 행은 별도 Added |
| 반대 방향 새 거래 | isCancellation=false인 원 행과 새 행을 각각 유지 |
| 같은 응답 반복 | 상태 동일, Unchanged만 반환 |
| 이력보다 앞선 동기화 시각·상한 초과 | 오류. 입력 집합은 변경되지 않음 |

모든 연산은 순수 함수다. 결과는 Reconciliation과 닫힌 RowChange 열거형이다. 현재 결제 집합은 disappeared_at이 없는 rows이고 임시 목록은 pending이다. 저장된 결제 행의 수는 줄지 않는다. 행과 모든 시각·상태·응답 변화는 공개 접근자로 읽으며 try_restore로 저장소에서 복원한다. 미결제는 R-01의 별도 임시 테이블 방식을 따르므로 영구 이력 행 수 보존 대상이 아니다. 벡터 순서는 식별 의미가 없고 순서 무관성은 멀티셋 동등성으로 정의한다.

입력 응답·임시 목록·저장 이력은 각각 최대 4096행이다. 변화 목록은 저장 이력과 입력의 합 이하 및 임시 교체 1건으로 제한된다. 추가 시 보존 이력이 상한에 닿으면 전체 연산이 실패하며 삭제·축약하지 않는다. 선형 탐색 기반 O(n*m)로 상한 안에서만 동작한다.

## 미해결과 후속

- I-06·design은 재등장 시 동일 관측 여부를 정하지 않는다. 기존 사라짐을 지우지 않는 보수 정책을 선택했다. 물리적 거래 동일성은 주장하지 않는다.
- R-11의 취소 시 원거래 삭제 제안은 확정 I-06과 충돌하므로 적용하지 않는다. 원거래와 취소 행의 확정 연결 키도 없다. D-28·D-30은 이 데이터를 무조건 합산하지 말고 별도 미확인 처리를 설계해야 한다.
- 현재 4096행은 영구 계좌 전체 규모를 보장하지 않는다. S-03은 계좌·날짜 구간으로 나누어 복원·대조하고 상한 초과를 명시적으로 처리해야 한다. 장기 이력의 분할 정책은 후속 설계 사항이다.
- AssetClass와 ISIN은 거래 시점 원문 코드다. ISIN 체크디지트, 상품별 필수 필드 조합, 알 수 없는 미래 상품 의미는 어댑터 계약 및 후속 분류에서 확인한다. 기존 Price는 0을 거부하므로 0 단가 원본이 관측되면 타입 정책 검토가 필요하다.
- 일대일 미결제→결제 연결은 국내 합산 기준 변경 때문에 만들지 않는다. 과거 미결제 원본 응답 보관은 S-03 책임이다.
- D-05·D-28·D-30의 계산·분류, 실제 키 실측, IO·스케줄·저장소는 이 작업 범위 밖이다.

## 검증 기준

- `reconciliation_matches_independent_sorted_counts`: Config::with_cases(256). 네 종류의 0~19행 입력을 생성하고 정렬·개수 세기 독립 기준과 대조한다. 현재 개수·보존 행 수·입력 순서 무관성·멱등성을 확인한다.
- `every_duplicate_count_pair_is_exact`: n,m 각각 0~16의 289개 조합을 전수 확인한다. 모든 자연수에 대한 수학적 증명이 아니라 경계 포함 유한 조합 검사다.
- `duplicate_counts_disappearance_reappearance_and_restore`: 3→2→3에서 사라진 시각 유지, 이력 행 수 4, 복원 왕복.
- `provisional_aggregate_is_replaced_without_guessing_settlement_links`: 미결제 1→결제 2에서 임시 집합 제거, 영구 결제 2행만 생성.
- `cancellation_preserves_original_and_distinguishes_reverse_transaction`: 원 행 사라짐과 취소 행 추가, 반대 방향 새 거래는 원 행 유지.
- `kind_codes_are_sealed_and_all_codes_round_trip`: 11개 알려진 코드와 미래·소문자 코드 왕복, 문자열 상한.
- `restoration_and_synchronization_bounds_are_atomic`, `synchronization_time_and_window_are_checked`, `synchronization_window_preserves_outside_rows`, `combined_capacity_failures_preserve_history`: 복원·날짜 창·이력 시각·4096/4097 경계와 병합 상한 오류의 원본 보존.

외부 실계좌·실제 키와 주문 경로는 실행하지 않는다. 이 테스트는 도메인 입력 계약을 검증하며 실제 서버에서 같은 응답이 오는지 증명하지 않는다.
