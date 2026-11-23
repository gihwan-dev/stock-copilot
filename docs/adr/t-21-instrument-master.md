# T-21: 전체 종목 마스터 공급

- 상태: 초안

## 결정

`BrokerRead::instruments`의 결과를 `InstrumentMaster`로 바꾼다. 각 행은 사용 가능한 `MasterInstrument` 또는 원본과 판정 사유를 보존한 사용 불가 행이다. 하나라도 사용 불가면 `complete_facts`는 실패한다. 응답의 행을 조용히 빼거나 일부를 완전 마스터로 바꾸지 않는다. 중복 심볼과 ID는 거부한다. D-52의 판정은 복사하지 않고 대응 포트의 `resolve`와 `ensure`를 쓴다.

현재 토스 문서만으로 상태 기본값의 전체 범위와 종목 이력의 연속성을 증명할 수 없다. 기본 `TossAccountRead`는 네트워크나 원장 접근 전에 `MasterNotReady(StatusCoverage)`로 실패한다. 준비되지 않은 상태는 `Bug`가 아니라 대기로 분류한다. 구현된 조립 경로 `TossInstrumentRead`는 `InstrumentMasterAuthority`의 세션과 독립 `InstrumentIdIssuer`를 주입받는다. 세션은 상태 범위와 연속성을 증명할 수 있을 때만 시작하며, 목록과 상세를 모두 받은 후 원본·상장 거래소·상태·관측 시각 목록을 `verify_observation`으로 확인한다. 확인부터 `resolve`·`ensure`가 끝날 때까지 폐지·갱신과 같은 직렬화 경계를 유지해야 한다. 현재 제품용 세션과 발급원은 없다. 테스트 가짜는 이 외부 사실을 증명하지 않는다.

후보가 필요한 원본에만 발급원을 호출하며 심볼이나 ISIN을 전달하지 않는다. 이미 있는 원본은 기존 ID를 쓴다. 신규 대응의 `IdAlreadyUsed`, `IdentityUncertain`, `Retired`는 각 행에 남긴다. 원장·발급원 오류는 준비되지 않은 사유로 전체 실패한다. 매핑 전 상세 행의 생성 가능 여부를 확인할 때만 고정 검증용 ID를 사용하며 원장에 전달하거나 발급 결과로 노출하지 않는다.

한국은 KOSPI·KOSDAQ·KR_ETC, 미국은 NYSE·NASDAQ·AMEX·US_ETC를 각각 조회한다. 저장된 명세상 `/stocks/all`은 페이지가 없으며 각 요청은 기존 STOCK_ALL 제한기를 거친다. 각 거래소 응답은 최대 8192행으로 제한하고, 목록 전체의 중복 심볼은 닫힌 실패로 처리한다. `/stocks`는 최대 200개씩 조회한다. 어떤 거래소나 상세 묶음이 실패하거나 상세가 빠지거나 중복되면 목록을 반환하거나 원장에 발급하지 않는다. 목록과 상세의 원문 심볼·ISIN·거래소·이름·종류·보통주 여부가 다르면 실패한다. 원장에는 원문 심볼을 주고 마스터에는 기존 규약대로 대문자 심볼을 사용한다. 목록 부재를 폐지 증거로 삼지 않는다.

| 응답 필드 | 도메인 값 |
|---|---|
| `/stocks/all`의 요청 거래소 및 `symbol`, `isinCode` | 국가 시장, 원문 `Symbol`, `IsinCode`를 담은 `InstrumentSource` |
| 대응 포트의 ID | `MasterInstrumentRow.instrument_id` |
| `/stocks`의 `market`, `symbol`, `name`, `securityType` | `listing_market`, 정규형 `symbol`, `name`, `security_type` |
| `/stocks`의 `currency`, `status`, `isCommonShare` | `currency`, `listing_status`, `common_share` |
| `/stocks`의 `koreanMarketDetail` | 정리매매·KRX 정지·NXT 지원과 정지 상태 |
| 주입 시계 | `observed_at` |

섹터 구성과 ETF는 완전한 마스터를 주입받아 기존 검증을 쓴다. 새 조립 경로는 마스터에 필요한 종목이 없을 때 `MasterNotReady(MissingInstrument)`를 반환한다. 기존 `TossAccountRead`를 직접 사용하는 섹터 호출은 이전 `Unsupported(Instruments)` 계약을 유지하므로 새 조립 경로의 분류와 구별된다.

## 실측 대기

- `/stocks/all`의 `ACTIVE` 기본값이 필요한 전체 상장 상태를 포함하는지, 각 거래소와 상세 행이 같은 관측을 가리키는지.
- 심볼 재사용, 상장폐지·재상장, ISIN 변경 및 관측 사이 누락을 확인하고 연속성을 보장하는 방법.
- 실제 토스 응답의 필드·규모와 조회 중 변경 표현. 이번 테스트는 합성 서버만 사용한다.

## 인계

- S-21: 영속 원장과 함께 상태 범위·이력 연속성을 실제 근거로 확인하는 세션을 제공하고, 조회·발급·폐지와 검증을 같은 직렬화 경계로 통합한다. 기존 발급 ID의 편입, 재시작·경쟁·실패 복구를 검증한다. 근거가 없으면 세션은 계속 실패해야 한다.
- D-55: 후보 ID가 독립 발급원에서 나왔음을 타입으로 강제하고 전역 유일성 및 확인된 식별자 변경 이력을 정의한다. 현재 포트와 가짜는 독립성을 타입으로 인증하지 않는다.

## 검사와 손 변형

합성 서버와 기록된 OpenAPI 스키마를 사용한다. 세션 신뢰와 프로젝트 신뢰는 각각 확인하지 못했다. Git 훅 설치와 실제 실행 결과는 리뷰 기록에 별도로 남긴다.

## 사전 승인된 기존 테스트 변경

| 파일·대상 | 전 → 후 | 근거 |
|---|---|---|
| `application/tests/error_class.rs`의 `expected_class` | 기존 닫힌 오류 분기 → `MasterNotReady` 대기 분기 추가 | 사전 승인 (a), 기존 단언 유지 |
| `application/tests/ports.rs`의 domain import | 더 이상 반환형으로 쓰지 않는 `MasterInstrumentFacts` import 제거 | 사전 승인 (b), 기존 단언 유지. 검사에서 제거와 shadowing 두 건으로 표시 |
| 같은 파일의 `FakeBroker` 구현 | 반환형·준비값 `MasterInstrumentFacts` → `InstrumentMaster` | 사전 승인 (b)·(d) |
| 같은 파일의 `a20_broker_contract_methods_return_domain_results_and_send_futures` | 결과 타입 표기 변경 | 사전 승인 (b), 기존 단언 유지 |
| 같은 파일의 `a20_success_keeps_bound_facts_incomplete_orders_and_unknown_server_times` | 새 결과를 `complete_facts`로 변환한 뒤 기존 단언 | 사전 승인 (b) |
| 같은 파일의 `a20_master_fee_and_calendar_keep_both_requested_markets` | 새 결과를 `complete_facts`로 변환한 뒤 기존 단언 | 사전 승인 (b) |

`test-diff`는 위 항목만 7건으로 지적했다. 실제 기대값과 단언 강도는 바꾸지 않았다.

프로젝트 신뢰와 세션 훅 신뢰·적용 여부는 각각 확인하지 못했다. 지시문을 읽기 전에 상위 작업 지침에 따라 홈의 메모리 색인을 한 차례 검색했으며, 지시문을 읽은 뒤에는 홈 설정·메모리·자격 증명을 읽지 않았다. 기존 반환형 사용처의 컴파일 오류와 프런트 산출물 누락을 고쳐 테스트 목록 비교를 다시 실행했다.

첫 수동 `check-fast`는 정적 Clippy 오류가 별도로 확인되어 전체 승격 검사 중 이 작업에서 시작한 프로세스 그룹을 중단했다. `temp-audit`가 해당 실행의 미종료 자손과 임시 항목 다섯 개를 감지하고 자체 임시 디렉터리를 제거했다. 이 중단된 실행은 통과 증거로 세지 않는다. 이후 적용한 수정은 검사를 끄지 않고 좁은 범위의 Clippy와 회귀 테스트로 확인한다.

두 번째 수동 `check-fast`는 전체 테스트 2450개와 계측 테스트 1230개가 통과한 뒤, 유효한 심볼을 대문자로 바꾸면서 다시 검사하는 도달 불가능한 오류 분기 한 곳에서 커버리지가 실패했다. 심볼 타입 내부에서 검증된 문자의 대문자 변환으로 바꾸어 해당 분기를 제거했다. 이전 검사는 전체 통과로 세지 않는다.
