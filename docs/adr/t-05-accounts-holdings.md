# T-05 계좌 목록과 보유 주식 조회

- 상태: 초안

## 결정

기존 TokenManager.get만 호출하는 TossAccountRead를 둔다. 동일한 TossHttpClient의 T-03 제한기를 사용하며 별도 HTTP 전송 경로는 만들지 않는다. 계좌는 BrokerAccountRow::from_broker_account_result와 BrokerAccountRows::try_new를 통해 서버 순서, 유형, 관측 시각, 마스킹된 표시를 결합한다. 중복 계좌는 전체 오류다. 미지 유형은 ADR 0061대로 원문을 Unknown으로 보존하며 자동·명시 선택 단계에서만 거부한다. 실제 자료와 실제 API를 사용하지 않는다.

보유 변환에는 기존 MasterInstrumentFacts를 생성자에서 주입한다. 보유 응답의 국가·심볼·통화가 같은 마스터를 찾고 매도 방향의 게이트로 ResolvedOrderInstrument를 얻는다. 보유 응답에 없는 상장시장·상장상태·종목유형·정지상태·안정적 내부 ID는 발명하지 않는다. 누락·불일치·게이트 거부가 하나라도 있으면 조회 전체를 InvalidResponse(Validation)로 닫는다. 일부 행을 버리거나 0원·빈 보유로 바꾸지 않는다. 이 방식은 기존 포트 변경을 요구하지 않는다.

상장폐지·KRX 거래정지·미지 유형은 현재 공개 생성 경로가 거부한다. 정리매매는 매도 방향 식별이 허용되는 경우 보유 조회가 가능하다. 기존 주문 봉인은 유지한다. 부분 성공 보존이나 거래 불가 종목의 성공 반환은 별도 domain 설계가 필요하다. BrokerCapabilities는 현재 구현하지 않은 선택 기능을 모두 Unsupported로 선언한다. 후속 읽기 메서드는 HTTP를 보내지 않고 명시적 미지원 오류를 반환한다.

금액과 수량은 domain NumericInput과 Money/Price/Quantity/Ratio 생성자에서만 읽는다. 부동소수 변환과 어댑터 계산은 없다. 개별 행의 currency 및 요약의 krw/usd 키를 통화 근거로 사용한다. 요약 usd의 null과 키 누락은 모두 부재로 보존한다. Price 스키마의 required는 krw뿐이다. 필수 필드 누락·통화 부재·미지 국가/통화·정밀도 손실·중복 종목은 실패한다. 생성 타입 역직렬화·재직렬화로 얻은 알려진 필드만 숫자 검증한다. 원문에 새 필드가 있어도 생성 모델이 쓰지 않는 필드는 무시한다. 보존하지 않는 알려진 비용·세후 수치도 정확한 십진수인지 검증한다. 기존 ReportedHolding/ReportedPortfolio가 표현하는 세전 amount와 rate만 반환하며 비용·세후 값의 새 domain 필드는 만들지 않는다.

## 응답 형식 근거

저장소 docs/toss-openapi-1.2.28.json만 사용했다. 외부 문서·실제 API는 사용하지 않았다.

| 엔드포인트 | 포트 | 합성 녹화 | 스펙 근거 |
|---|---|---|---|
| GET /api/v1/accounts | BrokerRead::accounts | t05-accounts.json, t05-accounts-empty.json | paths의 해당 get.responses.200.content.application/json.examples.brokerageAccount 및 emptyAccounts, components.schemas.Account |
| GET /api/v1/holdings | BrokerRead::holdings | t05-holdings.json, t05-holdings-empty.json | paths의 해당 get.responses.200.content.application/json.examples.withHoldings 및 emptyHoldings, components.schemas.HoldingsOverview·HoldingsItem |

녹화는 tests/recordings에 origin=synthetic인 T-20 형식으로 추가했다. 계좌번호 00000000731은 합성값이다. 기존 accounts.json은 그대로 두었다. 모든 녹화는 기존 T-20 현재 스키마 검사를 통과해야 한다. 401·429는 account_recordings의 t05-accounts-401.json, t05-accounts-429.json, t05-holdings-401.json, t05-holdings-429.json을 사용한다. components.responses.Unauthorized의 tokenRevoked 예제 및 RateLimitExceeded 예제의 합성 사본이며 requestId만 합성 문자열로 바꾼다. 계좌·기본 보유·오류 녹화8개는 새 시험에서 직접 T-20 검증기에 대조한다. 국내 보유와 선택 USD 누락 녹화2개는 기존 자동 열거와 새 USD 회귀에서도 검사한다. 가짜 서버 시퀀스는 토큰 재발급·제한기 경로와 최소 Retry-After 대기를 함께 검증한다.

## 실측 대기

- 실제 accountNo의 길이·구분자 유무: 기존 domain의 숫자 8~20자리 정책 밖이면 오류다. T-13에서 확인한다.
- 보유 응답에는 상장시장·상태·종목유형·거래정지·안정적 내부 ID가 없다. 실측으로 추측해 채울 필드가 아니며 T-09의 마스터 조회/내부 ID 대응이 공급해야 한다.
- 실제 환경의 빈 보유·혼합 통화·null 요약 형식과 401·429 발생은 실측하지 않았다. 합성 계약 증거와 실제 연결 확인을 구분한다.

## 인계

- T-09: 실제 MasterInstrumentFacts 공급과 갱신을 조립에 연결한다. T-05를 선행으로 둔 순서는 어댑터 코드 의존과 실제 조립 의존을 구분해 메인이 조정한다. 합성 주입 시험은 제품의 실제 마스터 공급 완료를 뜻하지 않는다.
- D-33/A-06: 거래 불가 보유까지 성공 결과에 보존해야 한다면 보유 식별과 주문 적격성을 분리하는 선행 설계를 한다. 현재는 전체 실패이며 포트 변경 없이 부분 성공을 가장하지 않는다.
- T-06/T-07/T-11: 이 구현체의 해당 미구현 메서드를 기존 경로로 확장한다.
- T-13: 실제 계좌번호 표시 형식을 사용자 검증으로 확인한다.

## 절차와 검사 기록

시작 frozen install과 lefthook install 성공. 프로젝트 설정 hooks=true는 확인했지만 프로젝트 신뢰와 현재 세션 훅 적용은 확인 불가다. 지시문의 예외에 따라 실제 Git 훅과 검사 결과를 증거로 삼는다.

최초 소스 작성을 테스트 작성보다 먼저 진행했다. 따라서 테스트 우선 절차를 준수했다고 보고하지 않으며 초기 red 증거는 없다. 이후 정리매매 게이트 손 변형 생존을 확인하고 회귀 테스트를 추가해 같은 변형의 컴파일 후 실패를 확인했다.

기존 테스트와 기존 impl 변경은 최종 패치에 없다. 빈 계좌 녹화는 tests/account_recordings에 두고 새 통합 시험에서 기존 T-20 검증·재생 도우미로 직접 검사한다. T-20의 기존 일반 녹화 시험은 수정하지 않았다. 의존성 추가와 검사 끄기는 없다.

보호 파일 harness/arch-check/rules.toml의 http-write-means 허용 목록에 tests/accounts_holdings.rs 하나만 등록한다. 기존 T-20 메타데이터 검증·재생 도우미를 호출하는 시험 경계이며 제품 전송을 추가하지 않는다. 해당 파일 등록 승인은 브리프의 새 시험 파일 한정 허용에 따른다. Codex 설정, sandbox 잠금 조각, Claude 훅 해시는 공식 생성기로만 재생성한다.


## 구현 리뷰 수정

B1: 기존 domain에 미지 계좌 유형 보존 계약이 이미 있으므로 닫힌 실패 대상을 잘못 적용한 선행 거부를 제거했다. BROKERAGE와 NEW_KIND를 함께 반환하고 자동 선택은 BROKERAGE를 고르며 미지 계좌 명시 선택은 거부한다.

I1: 스키마가 허용하는 추가 응답 필드를 임의로 숫자 처리하지 않는다. 생성 모델에 남는 알려진 필드만 검증하며 원문의 알려진 필드에서 도메인 값을 만든다. 추가 문자열·불린·중첩 객체가 결과를 바꾸지 않는 회귀와 기존 commission 형식 오류 거부를 함께 유지한다.

M2: 로컬 권한 거부는 Forbidden, 잘못된 자격 증명은 InvalidCredentials, 엔드포인트·클라이언트 설정 오류는 MalformedRequest, 저장 자료 무결성·형식 오류는 InvalidResponse(Validation)로 옮긴다. 저장소의 일시 불가에는 원래 RetryDelay를 유지한다. Broker PortError에는 SystemDiagnostic이 없어 세 무결성 원인을 개별 보존하지 못하며 A-05 후속 계약에 인계한다. 실제 화면 문구 검증은 하지 않았다.

새 회귀3개는 e54f7e8 소스에서 컴파일 후 모두 실패했고 수정 후 모두 통과했다. 초기 구현의 테스트 우선 절차 미준수와 이 수정 회차의 red/green 증거를 구분한다. 수정 후 어댑터 전체135개와 Clippy가 통과했으며 RetryDelay 보존 시험을 추가했다.

## 3회차 비판 이후 마지막 수정

대상947cb731e8fbb084b2d3145dec6db7179b86e823의 중요 R3-1과 사소 R3-2는 새 Codex가 독립 실행으로 확정했다. 선택 USD 누락은 여섯 요약 위치 중 실제 amounts 변환을 하는 네 위치에서 실패한다는 것으로 범위를 정정했다. 부모는 두 회귀를 먼저 추가해 컴파일 후2개 모두 실패함을 확인하고 수정했다.

USD 키 부재는 null과 같은 None이다. 국내 보유의 두 합성 녹화는 OpenAPI의 filteredBySymbol 예제에서 왔으며, usd-omitted 녹화는 선택 키6개만 제거했다. 필수 KRW 누락, 잘못된 USD 문자열·숫자·불린·객체·지수 표기·표현 불가 정밀도는 계속 거부한다.

최종 오류의 Retry-After는 로컬 스펙과 T-03이 정의한 비어 있지 않은 ASCII 십진 정수 초만 보존한다. u64 파싱 및 domain TimeSpan 생성에 성공한 값은 Known으로 넘긴다. 0과 선행0도 원래 초 값을 유지하며 내부 백오프를 서버 값에 더하지 않는다. 누락·다른 표기·u64/TimeSpan 범위 초과는 Unknown이다. 제한기의 원문 기한·백오프·과대 값 처리에는 손대지 않는다.

계좌 번호 하이픈과 현재가0 거부는 ADR0061·0038의 기존 계약에 따라 유지한다. 실제 허용 형식과 현재가0의 정상 응답 여부는 T-13 사용자 실측 사항이다. 3회차 실행으로 재현된 중요 문제를 수정하고4회차 독립 리뷰 없이 회귀·변이·최종 병합 검사로 마친다는 원지시문 예외를 적용한다. 이는 수정 후 독립 재리뷰를 통과했다는 뜻이 아니다.

마지막 수정의 좁은 검사18개와 Clippy가 통과했다. 전체 검사와 마지막 SHA는 정상 커밋/병합 훅의 실제 결과로 판단한다.

## 기존 속성 테스트 커버리지의 일회 누락

마지막 수정의 첫 정상 커밋 훅은 전체1676개와 커버리지 대상1059개 테스트가 모두 통과했지만, 변경하지 않은 crates/domain/src/tick_size.rs:273에서 coverage:uncovered-line으로 종료1이었다. 이 줄은 cfg(test)의 integer_reference_proves_tick_bounds_and_direction_for_4096_cases 참조 계산에서 coefficient % unit == 0인 경우에만 실행된다. 이 작업은 해당 파일을 바꾸지 않았다. 기존 테스트·난수 설정·반복 수·커버리지 기준을 바꾸지 않고 정상 커밋 훅을 재실행한다. 이 테스트의 결정적 경계 표본 보강은 D-35/H-11 인계다. 최초 실패 로그는 target/T-05/review3-commit.log에 보존한다. 두 번째 정상 훅은 기존 xtask의 hook_trace_distinguishes_full_from_fast 및 h44_review_real_auto_merge_still_rejects_work_test_changes에서 실패했다(1471개 실행,1469통과/2실패,205미실행). 소스 변경 없는 좁은 재실행은2/2 통과했으며 원인은 미확정이다. target/T-05/review3-commit-retry.log와 harness-regression-repro.log를 보존하고 같은 기준의 정상 훅을 다시 실행한다.

## 마지막 검증 범위

3회차 후 수정 범위는947cb731e8fbb084b2d3145dec6db7179b86e823 → 3f498f98c539116793be9875f0e769820fddf2c7다. 이 수정에4회차 독립 리뷰는 하지 않았다. 부모의 회귀·변이와 정상 커밋 훅을 증거로 직접 평가했다. 수정 커밋의 전체 검사는1676개, 커버리지 대상1059개가 통과했고 coverage-check/test-diff 위반은 없다.

최신 master afc9a3f9841832585bcc48fe122e4f36cd27b367를 별도 --no-ff 병합으로 수신한다. 병합 뒤 전체 검사 결과와 최종 SHA는 마지막 정상 훅 로그 target/T-05/merge.log 및 최종 보고에 연결한다. master 참조는 이동하지 않는다. 병합으로 받은 다른 작업의 변경과 T-05 변경을 구분한다.

## 손 변형

최종 제품 소스를 mktemp -d 복사본에 복사해 한 번에 하나씩 실제 컴파일·실행했다. 아래34개가 모두 컴파일 후 실패로 검출됐다. 생존0, 최종 컴파일 불가0이다. 첫 구현의 생존1개에는 정리매매 회귀를 추가했고 초기 컴파일 불가 변형은 검출로 세지 않은 뒤 컴파일 가능한 변형으로 교체해 재실행했다. 마지막 복사본과 부모의 account_read.rs·holding_read.rs·account_ports.rs 바이트 일치를 확인했고 복구 후 별도 명시적 unit3개와 integration15개도 통과했다. 처음23개·중간30개 증거와 마지막34개를 합산해 더 큰 검출 수로 보고하지 않는다.

자동 mutants-diff는 초기 중간 test-diff 실패를 최종 패치에서 제거한 뒤 종료0이었다. 현재 자동 examine_globs에 이 어댑터가 없어 선택 변이0개이며 검출 성공으로 세지 않는다. 마지막 master 기준 재실행 결과는 target/T-05/final-automatic.log와 최종 보고에 남긴다.

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|
| 1 | field quantity -> lastPrice | 컴파일 후 검출(exit 101) | tests::additional_response_fields_do_not_change_known_holdings, tests::holdings_preserve_each_distinct_field_and_request_account, tests::malformed_fields_fail_the_whole_response |
| 2 | field lastPrice -> quantity | 컴파일 후 검출(exit 101) | tests::additional_response_fields_do_not_change_known_holdings, tests::holdings_preserve_each_distinct_field_and_request_account, tests::malformed_fields_fail_the_whole_response |
| 3 | field averagePurchasePrice -> lastPrice | 컴파일 후 검출(exit 101) | tests::additional_response_fields_do_not_change_known_holdings, tests::holdings_preserve_each_distinct_field_and_request_account |
| 4 | field marketValue/amount -> profitLoss/amount | 컴파일 후 검출(exit 101) | tests::additional_response_fields_do_not_change_known_holdings, tests::holdings_preserve_each_distinct_field_and_request_account |
| 5 | field profitLoss/amount -> marketValue/amount | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::additional_response_fields_do_not_change_known_holdings |
| 6 | field profitLoss/rate -> dailyProfitLoss/rate | 컴파일 후 검출(exit 101) | tests::additional_response_fields_do_not_change_known_holdings, tests::holdings_preserve_each_distinct_field_and_request_account |
| 7 | field dailyProfitLoss/amount -> profitLoss/amount | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::additional_response_fields_do_not_change_known_holdings |
| 8 | field dailyProfitLoss/rate -> profitLoss/rate | 컴파일 후 검출(exit 101) | tests::additional_response_fields_do_not_change_known_holdings, tests::holdings_preserve_each_distinct_field_and_request_account |
| 9 | field totalPurchaseAmount -> marketValue/amount | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::additional_response_fields_do_not_change_known_holdings |
| 10 | KRW summary interpreted as USD | 컴파일 후 검출(exit 101) | tests::additional_response_fields_do_not_change_known_holdings, tests::liquidation_holdings_use_the_sell_identity_and_preserve_the_summary_rate, tests::holdings_preserve_each_distinct_field_and_request_account, tests::empty_recordings_remain_empty_successes, tests::revoked_token_and_rate_limit_keep_the_existing_paths, tests::optional_usd_omission_succeeds_for_every_summary_amount |
| 11 | USD summary interpreted as KRW | 컴파일 후 검출(exit 101) | tests::liquidation_holdings_use_the_sell_identity_and_preserve_the_summary_rate, tests::holdings_preserve_each_distinct_field_and_request_account, tests::additional_response_fields_do_not_change_known_holdings, tests::revoked_token_and_rate_limit_keep_the_existing_paths |
| 12 | sell gate replaced by buy | 컴파일 후 검출(exit 101) | tests::liquidation_holdings_use_the_sell_identity_and_preserve_the_summary_rate |
| 13 | duplicate guard reversed | 컴파일 후 검출(exit 101) | tests::additional_response_fields_do_not_change_known_holdings, tests::holdings_preserve_each_distinct_field_and_request_account, tests::liquidation_holdings_use_the_sell_identity_and_preserve_the_summary_rate, tests::revoked_token_and_rate_limit_keep_the_existing_paths, tests::optional_usd_omission_succeeds_for_every_summary_amount |
| 14 | symbol comparison reversed | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::liquidation_holdings_use_the_sell_identity_and_preserve_the_summary_rate, tests::additional_response_fields_do_not_change_known_holdings, tests::revoked_token_and_rate_limit_keep_the_existing_paths, tests::optional_usd_omission_succeeds_for_every_summary_amount |
| 15 | currency comparison reversed | 컴파일 후 검출(exit 101) | tests::liquidation_holdings_use_the_sell_identity_and_preserve_the_summary_rate, tests::additional_response_fields_do_not_change_known_holdings, tests::holdings_preserve_each_distinct_field_and_request_account, tests::revoked_token_and_rate_limit_keep_the_existing_paths, tests::optional_usd_omission_succeeds_for_every_summary_amount |
| 16 | market comparison reversed | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account, tests::additional_response_fields_do_not_change_known_holdings, tests::liquidation_holdings_use_the_sell_identity_and_preserve_the_summary_rate, tests::revoked_token_and_rate_limit_keep_the_existing_paths, tests::optional_usd_omission_succeeds_for_every_summary_amount |
| 17 | account key zero | 컴파일 후 검출(exit 101) | tests::accounts_preserve_key_kind_mask_order_and_time, tests::unknown_account_kind_is_preserved_without_hiding_brokerage |
| 18 | account type constant for nonempty code | 컴파일 후 검출(exit 101) | tests::accounts_preserve_key_kind_mask_order_and_time, tests::unknown_account_kind_is_preserved_without_hiding_brokerage |
| 19 | request account incremented | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account |
| 20 | holdings route replaced | 컴파일 후 검출(exit 101) | tests::holdings_preserve_each_distinct_field_and_request_account |
| 21 | accounts route replaced | 컴파일 후 검출(exit 101) | tests::accounts_preserve_key_kind_mask_order_and_time |
| 22 | success boundary excludes 200 | 컴파일 후 검출(exit 101) | tests::empty_recordings_remain_empty_successes, tests::holdings_preserve_each_distinct_field_and_request_account, tests::accounts_preserve_key_kind_mask_order_and_time, tests::additional_response_fields_do_not_change_known_holdings, tests::unknown_account_kind_is_preserved_without_hiding_brokerage, tests::liquidation_holdings_use_the_sell_identity_and_preserve_the_summary_rate, tests::revoked_token_and_rate_limit_keep_the_existing_paths, tests::optional_usd_omission_succeeds_for_every_summary_amount |
| 23 | envelope guard reversed | 컴파일 후 검출(exit 101) | tests::unresolved_and_suspended_master_fail_without_dropping_rows, tests::accounts_preserve_key_kind_mask_order_and_time, tests::unknown_account_kind_is_preserved_without_hiding_brokerage, tests::holdings_preserve_each_distinct_field_and_request_account, tests::empty_recordings_remain_empty_successes, tests::additional_response_fields_do_not_change_known_holdings, tests::liquidation_holdings_use_the_sell_identity_and_preserve_the_summary_rate, tests::revoked_token_and_rate_limit_keep_the_existing_paths, tests::missing_required_fields_and_duplicates_never_become_empty_success, tests::optional_usd_omission_succeeds_for_every_summary_amount |
| 24 | reject unknown account kind | 컴파일 후 검출(exit 101) | tests::unknown_account_kind_is_preserved_without_hiding_brokerage |
| 25 | validate unknown raw response fields | 컴파일 후 검출(exit 101) | tests::additional_response_fields_do_not_change_known_holdings |
| 26 | permission denied becomes network wait | 컴파일 후 검출(exit 101) | account_read::tests::local_token_failures_do_not_become_network_waits |
| 27 | invalid credentials become network wait | 컴파일 후 검출(exit 101) | account_read::tests::local_token_failures_do_not_become_network_waits |
| 28 | configuration error becomes network wait | 컴파일 후 검출(exit 101) | account_read::tests::local_token_failures_do_not_become_network_waits |
| 29 | invalid stored data becomes network wait | 컴파일 후 검출(exit 101) | account_read::tests::local_token_failures_do_not_become_network_waits |
| 30 | stored retry delay discarded | 컴파일 후 검출(exit 101) | account_read::tests::unavailable_store_preserves_its_retry_delay |
| 31 | optional USD omission rejected | 컴파일 후 검출(exit 101) | tests::optional_usd_omission_succeeds_for_every_summary_amount |
| 32 | valid server delay discarded | 컴파일 후 검출(exit 101) | account_read::tests::server_retry_delay_preserves_only_representable_decimal_seconds |
| 33 | signed server delay accepted | 컴파일 후 검출(exit 101) | account_read::tests::server_retry_delay_preserves_only_representable_decimal_seconds |
| 34 | final HTTP response delay discarded | 컴파일 후 검출(exit 101) | tests::exhausted_rate_limit_preserves_the_server_delay |
