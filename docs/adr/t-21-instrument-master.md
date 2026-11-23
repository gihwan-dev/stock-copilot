# T-21: 전체 종목 마스터 공급

- 상태: 초안

## 결정

`BrokerRead::instruments`의 결과를 `InstrumentMaster`로 바꾼다. 각 행은 사용 가능한 `MasterInstrument` 또는 원본과 판정 사유를 보존한 사용 불가 행이다. 하나라도 사용 불가면 `complete_facts`는 실패한다. 응답의 행을 조용히 빼거나 일부를 완전 마스터로 바꾸지 않는다. 중복 심볼과 ID는 거부한다. D-52의 판정은 복사하지 않고 대응 포트의 `resolve`·`ensure`·`retire`를 쓴다. 한 조회 안의 원본끼리 겹치는 경우는 D-52가 쓰는 동일한 시장·심볼·ISIN 정규형을 도메인에서 묶어 관련된 모든 행을 `IdentityUncertain`으로 표시하고, 그 행들에 대한 원장 조회·발급·폐지를 하지 않는다. 순서에 따라 한쪽에 먼저 ID가 붙는 일을 막는다.

현재 토스 문서만으로 상태 기본값의 전체 범위와 종목 이력의 연속성을 증명할 수 없다. 기본 `TossAccountRead`는 네트워크나 원장 접근 전에 `MasterNotReady(StatusCoverage)`로 실패한다. 상태 범위·연속성·마스터 누락의 준비 대기는 `Bug`가 아니라 대기로 분류한다. 구현된 조립 경로 `TossInstrumentRead`는 `InstrumentMasterAuthority`의 세션과 독립 `InstrumentIdIssuer`를 주입받는다. 세션은 상태 범위와 연속성을 증명할 수 있을 때만 시작하며, 목록과 상세를 모두 받은 후 원본·상장 거래소·상태·관측 시각 목록을 `verify_observation`으로 확인한다. 이 확인은 명시된 폐지 상태도 원장에 기록해도 되는 확인 사실이어야 한다. 확인부터 `resolve`·`ensure`·`retire`가 끝날 때까지 폐지·갱신과 같은 직렬화 경계를 유지해야 한다. 현재 제품용 세션과 발급원은 없다. 테스트 가짜는 이 외부 사실을 증명하지 않는다.

후보가 필요한 원본에만 발급원을 호출하며 심볼이나 ISIN을 전달하지 않는다. 이미 있는 원본은 기존 ID를 쓴다. 신규 대응의 `IdAlreadyUsed`, `IdentityUncertain`, `Retired`는 각 행에 남긴다. 확인된 `DELISTED`는 기존 대응에 `retire`를 호출하고 사용 불가로 남긴다. 원장에 없던 폐지 원본은 새 ID를 발급하지 않고 `IdentityUncertain`으로 남긴다. 원장·발급원 오류는 전체 실패하되 `SystemPortError`를 보존해 권한, 무결성, 일시 장애의 원래 분류를 잃지 않는다. 각 원장 연산은 원자적이지만 목록 전체가 하나의 트랜잭션은 아니다. 뒤 행이 실패하면 앞에서 발급한 대응은 남으며 재시도 때 그 ID를 재사용한다. 매핑 전 상세 행의 생성 가능 여부를 확인할 때만 고정 검증용 ID를 사용하며 원장에 전달하거나 발급 결과로 노출하지 않는다.

한국은 KOSPI·KOSDAQ·KR_ETC, 미국은 NYSE·NASDAQ·AMEX·US_ETC를 각각 조회한다. 저장된 명세상 `/stocks/all`은 페이지가 없으며 각 요청은 기존 STOCK_ALL 제한기를 거친다. 각 거래소 응답은 최대 8192행으로 제한하고, 목록 전체의 중복 심볼은 닫힌 실패로 처리한다. `/stocks`는 최대 200개씩 조회한다. 어떤 거래소나 상세 묶음이 실패하거나 상세가 빠지거나 중복되면 목록을 반환하거나 원장에 발급하지 않는다. 목록과 상세의 원문 심볼·ISIN·거래소·이름·종류·보통주 여부가 다르면 실패한다. 원장에는 원문 심볼을 주고 마스터에는 기존 규약대로 대문자 심볼을 사용한다. 목록 부재를 폐지 증거로 삼지 않는다.

| 응답 필드 | 도메인 값 |
|---|---|
| `/stocks/all`의 요청 거래소 및 `symbol`, `isinCode` | 국가 시장, 원문 `Symbol`, `IsinCode`를 담은 `InstrumentSource` |
| 대응 포트의 ID | `MasterInstrumentRow.instrument_id` |
| `/stocks`의 `market`, `symbol`, `name`, `securityType` | `listing_market`, 정규형 `symbol`, `name`, `security_type` |
| `/stocks`의 `currency`, `status`, `isCommonShare` | `currency`, `listing_status`, `common_share` |
| `/stocks`의 `koreanMarketDetail` | 정리매매·KRX 정지·NXT 지원과 정지 상태 |
| 주입 시계 | `observed_at` |

섹터 구성과 ETF는 완전한 마스터를 주입받아 기존 검증을 쓴다. 새 조립 경로는 보유·수정 일봉·섹터 구성·ETF에 필요한 종목이 마스터에 없을 때 `MasterNotReady(MissingInstrument)`를 반환한다. ETF 응답과 마스터에 같은 종목이 있지만 유형이 다르면 응답 검증 오류로 구별한다. 기존 `TossAccountRead` 직접 호출의 오류 계약은 그대로 둔다. 기본 어댑터의 Instruments capability는 Unsupported지만 호출 시에는 T-21의 실측 대기 사유를 돌려주는 차이가 있다.

보유 응답은 전체 행의 시장별 심볼 문법과 중복, 수량·취득가·현재가·보고 평가금액의 의미를 마스터 조회 전에 검증한다. 유효한 행에 대응이 없을 때만 마스터 미준비로 분류한다. 심볼 검증은 섹터 등 기존 시장 조회와 같은 도메인 규칙을 재사용한다. 미국 소문자를 새로 정규화해 다른 원본과 동일하다고 추정하지 않는다. 한 시장씩 따로 관측할 때 먼저 발급한 원본은 이후 다른 시장의 ISIN 충돌만으로 소급 변경하지 않으며, 뒤 원본은 D-52대로 `IdentityUncertain`이다.

## 실측 대기

- `/stocks/all`의 `ACTIVE` 기본값이 필요한 전체 상장 상태를 포함하는지, 각 거래소와 상세 행이 같은 관측을 가리키는지.
- 심볼 재사용, 상장폐지·재상장, ISIN 변경 및 관측 사이 누락을 확인하고 연속성을 보장하는 방법.
- 실제 토스 응답의 필드·규모와 조회 중 변경 표현. 이번 테스트는 합성 서버만 사용한다.

## 인계

- S-21: 영속 원장과 함께 상태 범위·이력 연속성을 실제 근거로 확인하는 세션을 제공하고, 조회·발급·폐지와 검증을 같은 직렬화 경계로 통합한다. 요청 거래소·상태 필터·조회 시작과 종료 및 빈 거래소의 증거를 인증 입력에 포함하고, 네트워크 대기 중 잠금과 발급원 재진입·취소 시 해제를 검증한다. 원장에 없는 폐지 원본의 이력과 이후 재상장도 빠짐없이 증명해야 한다. 기존 발급 ID의 편입, 재시작·경쟁·실패 복구를 검증한다. 근거가 없으면 세션은 계속 실패해야 한다.
- D-55: 후보 ID가 독립 발급원에서 나왔음을 타입으로 강제하고 전역 유일성 및 확인된 식별자 변경 이력을 정의한다. 현재 포트와 가짜는 독립성을 타입으로 인증하지 않는다.
- T-05·A-06 후속: 보유 사실 생성이 기존 `ResolvedOrderInstrument`의 매도 게이트를 요구해 정지·비활성·미지 유형 종목의 정상 보유를 조회 오류로 만들 수 있다. 보유 식별과 주문 허용 판정을 분리하는 타입·호출부를 검토한다. 주문 게이트 자체를 완화하지 않는다.

`entries()`에서 사용 가능 행만 새 `MasterInstrumentFacts`로 재구성하는 일은 공개 생성자가 막지 않는다. 이 생성자는 외부 응답의 진위나 목록 완전성을 인증하지 않는다. 상위 `ErrorClass`와 IPC는 준비 대기를 일반 대기와 합치므로 화면 소비가 생기면 마스터 사유를 전달하는 계약이 필요하다.

## 검사와 손 변형

합성 서버와 기록된 OpenAPI 스키마를 사용한다. 세션 신뢰와 프로젝트 신뢰는 각각 확인하지 못했다. Git 훅 설치와 실제 실행 결과는 리뷰 기록에 별도로 남긴다.

## 사전 승인된 기존 테스트 변경

| 파일·대상 | 전 → 후 | 근거 |
|---|---|---|
| `application/tests/error_class.rs`의 `expected_class` | 기존 닫힌 오류 분기 → `MasterNotReady`의 준비 대기·시스템 오류 분기 추가 | 사전 승인 (a), 기존 단언 유지 |
| `application/tests/ports.rs`의 domain import와 `FakeBroker` 구현 | 기존 `MasterInstrumentFacts` 준비값과 import를 유지한 뒤 `InstrumentMaster` 행으로 변환 | 사전 승인 (b)·(d), 기존 단언 유지 |
| 같은 파일의 `a20_broker_contract_methods_return_domain_results_and_send_futures` | 결과 타입 표기 변경 | 사전 승인 (b), 기존 단언 유지 |
| 같은 파일의 `a20_success_keeps_bound_facts_incomplete_orders_and_unknown_server_times` | 새 결과를 `complete_facts`로 변환한 뒤 기존 단언 | 사전 승인 (b) |
| 같은 파일의 `a20_master_fee_and_calendar_keep_both_requested_markets` | 새 결과를 `complete_facts`로 변환한 뒤 기존 단언 | 사전 승인 (b) |

첫 구현 커밋에 대한 `test-diff`는 7건을 지적했다. 이후 원래 import를 유지하는 준비 코드로 바꾼 작업 트리의 정적 비교에서는 import 제거·shadowing이 사라지고 5건이 남는다. 실제 기대값과 단언 강도는 바꾸지 않았다. 다만 임시 복사본에서 승인 판정기를 직접 실행한 결과 나머지 5건 중 4건은 승인 행을 추가해도 현재 판정기가 거부했다. 실제 `master` 승인 및 정상 병합 훅은 별도로 필요하다.

| 파일 | 기준 SHA-256 | 현재 작업 트리 SHA-256 |
|---|---|---|
| `crates/application/tests/error_class.rs` | `d2565e475e827e5ef1bf9951921333e5b6951b4bab1c7259ef08f3e9f8dd4a07` | `00dec8ef03127245368532c9eddb3db185a61a6ab63ceda5cb34cbca557cdb97` |
| `crates/application/tests/ports.rs` | `bf07826edbf01a1464bae86a3a4f5dc33f3922ceaa1b523a7585c5b541fc216c` | `0c77ac28ecb424ef4b458eccb9a9fdde3fbd00e3c10a76dc8108b6a60f941f2d` |

프로젝트 신뢰와 세션 훅 신뢰·적용 여부는 각각 확인하지 못했다. 지시문을 읽기 전에 상위 작업 지침에 따라 홈의 메모리 색인을 한 차례 검색했으며, 지시문을 읽은 뒤에는 홈 설정·메모리·자격 증명을 읽지 않았다. 기존 반환형 사용처의 컴파일 오류와 프런트 산출물 누락을 고쳐 테스트 목록 비교를 다시 실행했다.

첫 수동 `check-fast`는 정적 Clippy 오류가 별도로 확인되어 전체 승격 검사 중 이 작업에서 시작한 프로세스 그룹을 중단했다. `temp-audit`가 해당 실행의 미종료 자손과 임시 항목 다섯 개를 감지하고 자체 임시 디렉터리를 제거했다. 이 중단된 실행은 통과 증거로 세지 않는다. 이후 적용한 수정은 검사를 끄지 않고 좁은 범위의 Clippy와 회귀 테스트로 확인한다.

두 번째 수동 `check-fast`는 전체 테스트 2450개와 계측 테스트 1230개가 통과한 뒤, 유효한 심볼을 대문자로 바꾸면서 다시 검사하는 도달 불가능한 오류 분기 한 곳에서 커버리지가 실패했다. 심볼 타입 내부에서 검증된 문자의 대문자 변환으로 바꾸어 해당 분기를 제거했다. 이전 검사는 전체 통과로 세지 않는다.

1차 리뷰 수정 뒤 별도 도메인 커버리지 검사는 계측 테스트 1233개가 통과했지만 세 번째 중복 관측 분기 한 곳에서 실패했다. 그 관측을 추가한 재검사는 계측 테스트 1234개 통과와 위반 없음으로 끝났다. 이 검사는 정상 커밋 훅이나 최종 전체 검사의 영수증이 아니다.

## 손 변형

수정된 작업 트리를 `mktemp` 복사본에 옮겨 변형마다 테스트를 실제 실행했다. 아래 30개는 모두 컴파일된 뒤 표시한 테스트에서 실패했다. ETF 유형 충돌의 첫 변형은 미사용 메서드 때문에 컴파일되지 않아 세지 않았고, 같은 행동을 바꾸는 다른 변형을 실행했다.

| 번호 | 실제 변형 | 검출한 테스트 |
|---|---|---|
| 1 | KR 거래소 누락 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 2 | 미국 거래소 누락 | `us_exchanges_preserve_source_case_and_use_the_existing_id` |
| 3 | 심볼 일치 조건 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 4 | ISIN 일치 조건 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 5 | 거래소 일치 조건 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 6 | 이름 일치 조건 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 7 | 종목 유형 일치 조건 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 8 | 보통주 일치 조건 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 9 | 상장 예정 상태를 활성으로 변경 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 10 | 활성 상태를 상장 예정으로 변경 | `authority_receives_listing_status_market_and_observation_time_before_mapping` |
| 11 | 정리매매 상태 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 12 | KRX 정지 상태 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 13 | NXT 지원 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 14 | NXT 정지 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 15 | 보통주 상태 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 16 | 상세 묶음 개수 비교 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 17 | 상세 심볼 소속 조건 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 18 | 상세 중복 조건 반전 | `every_exchange_and_detail_is_joined_without_losing_fields` |
| 19 | 심볼 정규화 제거 | `us_exchanges_preserve_source_case_and_use_the_existing_id` |
| 20 | 기존 ID 조회 실패로 변경 | `existing_ids_are_reused_without_consuming_candidates` |
| 21 | 8192 초과 비교를 동일 비교로 변경 | `review_limit_above_8192_fails_before_next_exchange` |
| 22 | 8192 초과를 이상으로 변경 | `review_limit_exactly_8192_proceeds_to_the_next_exchange` |
| 23 | 폐지 상태를 활성으로 변경 | `review_explicit_delisted_status_cannot_become_tradable` |
| 24 | 조회 전체 충돌 판정 반전 | `review_regression_same_isin_never_grants_either_ambiguous_source_an_id` |
| 25 | D-52 식별 겹침 OR을 AND로 변경 | `case_variants_never_become_distinct_new_sources` |
| 26 | 폐지 원장 갱신을 조회로 대체 | `review_regression_verified_delisting_retires_existing_mapping` |
| 27 | 이력 없는 폐지를 폐지 확정으로 분류 | `review_explicit_delisted_status_cannot_become_tradable` |
| 28 | 시스템 오류 분류를 대기로 변경 | `system_failures_keep_their_original_classification` |
| 29 | 보유 마스터 누락의 구분 제거 | `review_regression_holdings_separates_missing_master_from_bad_response` |
| 30 | ETF 유형 충돌을 누락으로 변경 | `review_regression_etf_type_conflict_is_not_a_missing_instrument` |

2차 수정 후 위 30개와 아래 11개를 다시 실행했다. 27번은 소진 `match`로 옮긴 뒤 변형 위치가 달라져 맞는 위치로 다시 실행했다. 30번의 첫 시도는 미사용 메서드 오류로 컴파일되지 않아 제외했고, 오류 사유를 바꾸는 다른 변형을 실행했다. 32번은 두 응답 행이 서로 가려 처음에는 생존했으므로 각 입력을 한 행으로 분리해 다시 실행했다. 재실행에서 41개 모두 해당 테스트가 검출했다. 기존 30개 중 테스트 보강과 무관한 것은 보강 전 같은 제품 소스에서 실행한 결과다. 재실행용 사본은 `target/T-21/` 아래에서 실행 직후 정리했다.

| 번호 | 실제 변형 | 검출한 테스트 |
|---|---|---|
| 31 | 보유 시장별 심볼 검증 제거 | `review2_bad_holdings_are_rejected_before_master_lookup` |
| 32 | 보유 심볼을 고정값으로 변경 | `review2_bad_holdings_are_rejected_before_master_lookup` |
| 33 | 보유 수량을 고정값으로 변경 | `review2_bad_holdings_are_rejected_before_master_lookup` |
| 34 | 보유 현재가를 고정값으로 변경 | `review2_bad_holdings_are_rejected_before_master_lookup` |
| 35 | 보유 취득가를 고정값으로 변경 | `review2_bad_holdings_are_rejected_before_master_lookup` |
| 36 | 보유 평가액을 고정값으로 변경 | `review2_bad_holdings_are_rejected_before_master_lookup` |
| 37 | 보유 중복 검사 제거 | `review2_bad_holdings_are_rejected_before_master_lookup` |
| 38 | 첫 행 검증 후 빈 마스터를 먼저 반환 | `review2_bad_holdings_are_rejected_before_master_lookup` |
| 39 | 발급원 실패를 저장소 오류로 변경 | `review2_issuer_error_preserves_its_origin_and_response_status` |
| 40 | 발급 실패의 원장 오류 사유 변경 | `review2_ensure_missing_is_a_repository_error_without_assignment` |
| 41 | 불확실 폐지를 확정 폐지로 변경 | `review2_uncertain_retirement_preserves_the_existing_mapping` |
