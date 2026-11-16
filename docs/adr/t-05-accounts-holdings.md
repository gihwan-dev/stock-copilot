# T-05 계좌 목록과 보유 주식 조회

- 상태: 초안

## 결정

기존 TokenManager.get만 호출하는 TossAccountRead를 둔다. 동일한 TossHttpClient의 T-03 제한기를 사용하며 별도 HTTP 전송 경로는 만들지 않는다. 계좌는 BrokerAccountRow::from_broker_account_result와 BrokerAccountRows::try_new를 통해 서버 순서, 유형, 관측 시각, 마스킹된 표시를 결합한다. 중복 계좌와 미지 유형은 전체 오류다. 실제 자료와 실제 API를 사용하지 않는다.

보유 변환에는 기존 MasterInstrumentFacts를 생성자에서 주입한다. 보유 응답의 국가·심볼·통화가 같은 마스터를 찾고 매도 방향의 게이트로 ResolvedOrderInstrument를 얻는다. 보유 응답에 없는 상장시장·상장상태·종목유형·정지상태·안정적 내부 ID는 발명하지 않는다. 누락·불일치·게이트 거부가 하나라도 있으면 조회 전체를 InvalidResponse(Validation)로 닫는다. 일부 행을 버리거나 0원·빈 보유로 바꾸지 않는다. 이 방식은 기존 포트 변경을 요구하지 않는다.

상장폐지·KRX 거래정지·미지 유형은 현재 공개 생성 경로가 거부한다. 정리매매는 매도 방향 식별이 허용되는 경우 보유 조회가 가능하다. 기존 주문 봉인은 유지한다. 부분 성공 보존이나 거래 불가 종목의 성공 반환은 별도 domain 설계가 필요하다. BrokerCapabilities는 현재 구현하지 않은 선택 기능을 모두 Unsupported로 선언한다. 후속 읽기 메서드는 HTTP를 보내지 않고 명시적 미지원 오류를 반환한다.

금액과 수량은 domain NumericInput과 Money/Price/Quantity/Ratio 생성자에서만 읽는다. 부동소수 변환과 어댑터 계산은 없다. 개별 행의 currency 및 요약의 krw/usd 키를 통화 근거로 사용한다. 요약 usd null은 부재로 보존한다. 필수 필드 누락·통화 부재·미지 국가/통화·정밀도 손실·중복 종목은 실패한다. 생성 타입 역직렬화 뒤 보존하지 않는 비용·세후 수치도 정확한 십진수인지 검증한다. 기존 ReportedHolding/ReportedPortfolio가 표현하는 세전 amount와 rate만 반환하며 비용·세후 값의 새 domain 필드는 만들지 않는다.

## 응답 형식 근거

저장소 docs/toss-openapi-1.2.28.json만 사용했다. 외부 문서·실제 API는 사용하지 않았다.

| 엔드포인트 | 포트 | 합성 녹화 | 스펙 근거 |
|---|---|---|---|
| GET /api/v1/accounts | BrokerRead::accounts | t05-accounts.json, t05-accounts-empty.json | paths의 해당 get.responses.200.content.application/json.examples.brokerageAccount 및 emptyAccounts, components.schemas.Account |
| GET /api/v1/holdings | BrokerRead::holdings | t05-holdings.json, t05-holdings-empty.json | paths의 해당 get.responses.200.content.application/json.examples.withHoldings 및 emptyHoldings, components.schemas.HoldingsOverview·HoldingsItem |

녹화는 tests/recordings에 origin=synthetic인 T-20 형식으로 추가했다. 계좌번호 00000000731은 합성값이다. 기존 accounts.json은 그대로 두었다. 모든 녹화는 기존 T-20 현재 스키마 검사를 통과해야 한다. 401·429는 녹화 본문에 오류 응답을 끼운 가짜 서버 시퀀스로 토큰 재발급·제한기 경로를 검증한다.

## 실측 대기

- 실제 accountNo의 길이·구분자 유무: 기존 domain의 숫자 8~20자리 정책 밖이면 오류다. T-13에서 확인한다.
- 보유 응답에는 상장시장·상태·종목유형·거래정지·안정적 내부 ID가 없다. 실측으로 추측해 채울 필드가 아니며 T-09의 마스터 조회/내부 ID 대응이 공급해야 한다.
- 실제 환경의 빈 보유·혼합 통화·null 요약 형식과 401·429 발생은 실측하지 않았다. 합성 계약 증거와 실제 연결 확인을 구분한다.

## 인계

- T-09: 실제 MasterInstrumentFacts 공급과 갱신을 조립에 연결한다. T-05를 선행으로 둔 순서는 어댑터 코드 의존과 실제 조립 의존을 구분해 메인이 조정한다. 합성 주입 시험은 제품의 실제 마스터 공급 완료를 뜻하지 않는다.
- D-33/A-06: 거래 불가 보유까지 성공 결과에 보존해야 한다면 보유 식별과 주문 적격성을 분리하는 선행 설계를 한다. 현재는 전체 실패이며 포트 변경 없이 부분 성공을 가장하지 않는다.
- T-06/T-07: 이 구현체의 해당 미구현 메서드를 기존 경로로 확장한다.
- T-13: 실제 계좌번호 표시 형식을 사용자 검증으로 확인한다.

## 절차와 검사 기록

시작 frozen install과 lefthook install 성공. 프로젝트 설정 hooks=true는 확인했지만 프로젝트 신뢰와 현재 세션 훅 적용은 확인 불가다. 지시문의 예외에 따라 실제 Git 훅과 검사 결과를 증거로 삼는다.

최초 소스 작성을 테스트 작성보다 먼저 진행했다. 따라서 테스트 우선 절차를 준수했다고 보고하지 않으며 초기 red 증거는 없다. 이후 정리매매 게이트 손 변형 생존을 확인하고 회귀 테스트를 추가해 같은 변형의 컴파일 후 실패를 확인했다.

기존 테스트와 기존 impl 변경은 최종 패치에 없다. 빈 계좌 녹화는 tests/account_recordings에 두고 새 통합 시험에서 기존 T-20 검증·재생 도우미로 직접 검사한다. T-20의 기존 일반 녹화 시험은 수정하지 않았다. 의존성 추가와 검사 끄기는 없다.

보호 파일 harness/arch-check/rules.toml의 http-write-means 허용 목록에 tests/accounts_holdings.rs 하나만 등록한다. 기존 T-20 메타데이터 검증·재생 도우미를 호출하는 시험 경계이며 제품 전송을 추가하지 않는다. 해당 파일 등록 승인은 브리프의 새 시험 파일 한정 허용에 따른다. Codex 설정, sandbox 잠금 조각, Claude 훅 해시는 공식 생성기로만 재생성한다.


## 손 변형

mktemp -d로 만든 별도 복사본에서 하나씩 실제 컴파일하고 accounts_holdings 통합 시험으로 확인했다. 최초 매수 게이트 변형은 생존했고 정리매매 보유 회귀를 추가한 뒤 검출됐다. 최초 컴파일 불가 두 변형은 집계에서 제외하고 컴파일 가능한 변형으로 교체해 재실행했다. 최종 23개가 컴파일 후 검출됐고 생존 0개다. 원본 및 최종 복구 검사도 통과했다.

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|
| 1 | field quantity -> lastPrice | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::malformed_fields_fail_the_whole_response |
| 2 | field lastPrice -> quantity | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::malformed_fields_fail_the_whole_response |
| 3 | field averagePurchasePrice -> lastPrice | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account |
| 4 | field marketValue/amount -> profitLoss/amount | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account |
| 5 | field profitLoss/amount -> marketValue/amount | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account |
| 6 | field profitLoss/rate -> dailyProfitLoss/rate | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account |
| 7 | field dailyProfitLoss/amount -> profitLoss/amount | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account |
| 8 | field dailyProfitLoss/rate -> profitLoss/rate | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account |
| 9 | field totalPurchaseAmount -> marketValue/amount | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account |
| 10 | KRW summary interpreted as USD | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::empty_recordings_remain_empty_successes, tests::revoked_token_and_rate_limit_keep_the_existing_paths |
| 11 | USD summary interpreted as KRW | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::revoked_token_and_rate_limit_keep_the_existing_paths |
| 12 | sell gate replaced by buy | 컴파일 후 검출(exit 101) | tests::liquidation_holdings_use_the_sell_identity_and_preserve_the_summary_rate |
| 13 | duplicate guard reversed | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::revoked_token_and_rate_limit_keep_the_existing_paths |
| 14 | symbol comparison reversed | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::revoked_token_and_rate_limit_keep_the_existing_paths |
| 15 | currency comparison reversed | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::revoked_token_and_rate_limit_keep_the_existing_paths |
| 16 | market comparison reversed | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::revoked_token_and_rate_limit_keep_the_existing_paths |
| 17 | account key zero | 컴파일 후 검출(exit 101) | tests::accounts_preserve_key_kind_mask_order_and_time |
| 18 | account type constant for nonempty code | 컴파일 후 검출(exit 101) | tests::accounts_preserve_key_kind_mask_order_and_time |
| 19 | request account incremented | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account |
| 20 | holdings route replaced | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account |
| 21 | accounts route replaced | 컴파일 후 검출(exit 101) | tests::accounts_preserve_key_kind_mask_order_and_time |
| 22 | success boundary excludes 200 | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::empty_recordings_remain_empty_successes, tests::accounts_preserve_key_kind_mask_order_and_time, tests::revoked_token_and_rate_limit_keep_the_existing_paths |
| 23 | envelope guard reversed | 컴파일 후 검출(exit 101) | tests::unresolved_and_suspended_master_fail_without_dropping_rows, tests::holdings_preserve_each_distinct_field_and_request_account, tests::empty_recordings_remain_empty_successes, tests::accounts_preserve_key_kind_mask_order_and_time, tests::revoked_token_and_rate_limit_keep_the_existing_paths, tests::missing_required_fields_and_duplicates_never_become_empty_success |
