# U-02 IPC 계약

- 상태: 초안

## 결정

기준 SHA는 520c8dfc0e5b2fdd9bcc69a1c32b48b0eff9c49a다. 기존 app_version 하나에 닫힌 current 조회 입력을 연결하고 기존 조립 루트에 UiEvent 하나를 등록한다. 입력은 serde deny_unknown_fields로 추가 필드를 거부한다. specta skip_attr = "serde"는 이 DTO의 serde 메타데이터만 생략한다. 필드명 변경이나 태그가 없는 DTO에서 TS 모양은 동일하며 serde의 런타임 거부는 유지한다. 수동으로 필드 목록을 복제하는 Type 구현보다 단순하고 끌 수 없는 disallowed_macros를 끄지 않는다. serde rename/tag/flatten을 추가하면 이 결정을 다시 검증해야 한다.

ErrorHandlingMode::Result를 유지한다. application ErrorClass의 네 분류를 user-fixable, retryable, bug, no-data로 옮긴다. 진단·대기 시간·원문·경로·계좌 키는 내보내지 않는다. no-data는 조회 빈 상태이며 상태 띠에 올리지 않는 처리는 U-04에 인계한다.

UiEvent는 외부 태그 열거형이다. 무페이로드는 종류 문자열, 나머지는 종류를 키로 갖는 객체다. 계좌 이벤트는 키를 버린다. 다른 ID는 문자열로 보존하며 공시는 소스와 접수 번호를 구분한다. 시장·소스·통화쌍은 닫힌 DTO 열거형으로 옮긴다. domain에 직렬화 의존성을 추가하지 않는다.

기존 큰 정수 거부 테스트의 i64/u64/i128/u128/isize/usize와 기본 Typescript exporter를 유지한다. 금액·수량 명령은 추가하지 않는다. 향후 해당 DTO는 문자열 십진수만 사용해야 한다.

H-13 생성기·재생성 바이트 비교를 유지하고 이벤트로 인해 달라진 고정 헤더·런타임 스냅샷과 해당 생성 파일의 허용 import만 갱신한다. 새 의존성·린트 끄기는 없다.

## 검증

프로젝트 신뢰와 세션 훅 신뢰는 확인 불가다. 저장소 hooks=true는 세션 적용 증거가 아니다. pnpm frozen 설치와 lefthook install은 성공했다. 검증 상세는 리뷰 기록에 남긴다.

## 인계

| 작업        | 내용                                                                                                                |
| ----------- | ------------------------------------------------------------------------------------------------------------------- |
| U-04        | no-data 빈 상태 처리, IPC 시간 제한과 쿼리 계층                                                                     |
| U-05 / A-10 | 이벤트 발행·구독과 선택 계좌 재조회 연결. 이번 작업은 타입 등록과 mount만 수행                                      |
| O-08        | 승인 입력에는 proposal_id만 허용. serde 메타데이터를 생략할 경우 태그·이름 변경을 쓰지 않고 직렬화 모양 회귀를 추가 |

## 승인된 기존 테스트 변경

| 파일                                   | 테스트                                                                       | 전 → 후                                                                      |
| -------------------------------------- | ---------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| src-tauri/src/bindings/tests.rs        | generated_typescript_names_the_command_mocks_and_recorded_responses          | 인자 없음 → AppVersionRequest 인자 서명                                      |
| src-tauri/src/ipc/tests.rs             | app_version_response_is_recorded_for_the_typescript_fake                     | app handle만 전달 → current 조회 DTO도 전달. 응답 단언 유지                  |
| src-tauri/src/ipc/tests.rs             | every_ipc_error_is_recorded_with_its_tag                                     | 세 분류 → NoData를 포함한 네 분류                                            |
| desktop/tests/generated-files.test.mjs | the generated file list holds exactly the IPC outputs of gen-bindings        | 두 생성 파일의 고정 import 목록에 event 한 항목 추가                         |
| desktop/tests/generated-files.test.mjs | generated files may import only their listed modules and nothing dynamically | 고정 허용 목록에 event가 추가된 정확한 오류 문구로 갱신. 다른 위반 단언 유지 |

사전 승인된 기존 테스트 변경은 5개다. Rust use 구문 변경에 따른 support-removed/shadowing 진단도 동일한 서명 변경의 기계적 결과다. 변경분 자동 변이는 test-diff에서 중단되어 실행하지 못했다. 손 변형 20개의 실제 실행으로 보완했으며 자동 변이 통과로 보고하지 않는다.

입력 DTO 추가 뒤 기존 AppHandle 명령의 needless_pass_by_value 예외가 모든 인자를 Tauri 주입 타입으로 제한하는 검사와 충돌했다. 명령을 async fn으로 바꾸어 인자를 명령 Future가 소유하도록 했다. 기존 needless_pass_by_value·unnecessary_wraps expect 두 개는 제거했고, 새 끄기 없이 Clippy가 통과했다. 별도 태스크·큐·상태는 추가하지 않았다. 기존 응답 테스트는 변경된 비동기 서명에 맞추어 block_on으로 호출하며 응답 단언을 유지한다.

## 커밋 전 막힘 (해소)

두 번째 정상 커밋 훅은 프런트 테스트 529개를 통과한 뒤 기존 Claude 훅 회귀 테스트에서 82 passed, 1 failed로 중단됐다. harness/claude-hooks/tests/allowlist.test.mjs:105는 docs/adr/0099-future.md에 대해 빈 위반 목록을 기대하지만 기준 커밋에는 이미 docs/adr/0099-composition-root.md가 있다. 실제 결과는 adr-number 위반이다. 기준의 테스트 본문과 ADR 존재를 확인했으며 단독 테스트로 재현했다.

이 fixture는 U-02 설계 변경으로 생긴 것이 아니다. 기존 테스트를 이번 작업 범위 밖에서 수정하지 않고 보존한다. 다른 검사 실패가 남아 있으므로 test-diff에 한정된 LEFTHOOK=0 예외도 사용하지 않았다. 구현은 미커밋 상태로 인계한다. 전체 검사 통과, 구현 커밋 이후의 독립 리뷰·비판은 미실행이다. 이 하네스 fixture 수정과 정상 검사 재실행이 재개 조건이다.

## 재개

메인이 afc9a3f9841832585bcc48fe122e4f36cd27b367에서 기존 세 하네스 테스트의 미사용 번호 예시를 9999로 수정했다. 해당 테스트를 직접 수정하지 않고 master를 정상 훅으로 병합했다. 수신 병합 커밋은 674d0a8f277d83afe81db3859b4c440268b5a0e9다. 병합 중 전용 stash에 보존한 구현 20개 파일은 --index로 복원했고 원래 스테이징 패치와 바이트 단위로 동일함을 확인했다. 전용 stash는 확인 뒤 제거했다. 기존 막힘은 해소되었고 정상 구현 커밋 검사와 독립 리뷰를 재개한다.

## 손 변형

| 번호 | 바꾼 것                                      | 잡은 테스트                                                                                                    | 결과 |
| ---- | -------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ---- |
| 1    | PortfolioSyncRecorded -> 다른 이벤트 종류    | ipc::event::tests::event_contract_group_0, ipc::event::tests::account_events_never_serialize_the_account_key   | 검출 |
| 2    | TransactionsSyncRecorded -> 다른 이벤트 종류 | ipc::event::tests::event_contract_group_0, ipc::event::tests::account_events_never_serialize_the_account_key   | 검출 |
| 3    | PriceUpdated -> 다른 이벤트 종류             | ipc::event::tests::event_contract_group_0, ipc::event::tests::non_account_identifiers_keep_their_exact_strings | 검출 |
| 4    | PriceMoved -> 다른 이벤트 종류               | ipc::event::tests::event_contract_group_3                                                                      | 검출 |
| 5    | OrderSubmitted -> 다른 이벤트 종류           | ipc::event::tests::non_account_identifiers_keep_their_exact_strings, ipc::event::tests::event_contract_group_9 | 검출 |
| 6    | BrokerConnectionChanged -> 다른 이벤트 종류  | ipc::event::tests::event_contract_group_12                                                                     | 검출 |
| 7    | SettingsChanged -> 다른 이벤트 종류          | ipc::event::tests::event_contract_group_12                                                                     | 검출 |
| 8    | PolicyChanged -> 다른 이벤트 종류            | ipc::event::tests::event_contract_group_12                                                                     | 검출 |
| 9    | OrderSwitchChanged -> 다른 이벤트 종류       | ipc::event::tests::event_contract_group_15                                                                     | 검출 |
| 10   | WatchlistChanged -> 다른 이벤트 종류         | ipc::event::tests::event_contract_group_15                                                                     | 검출 |
| 11   | ThesisUpdated -> 다른 이벤트 종류            | ipc::event::tests::event_contract_group_15                                                                     | 검출 |
| 12   | SelfCheckChanged -> 다른 이벤트 종류         | ipc::event::tests::event_contract_group_18                                                                     | 검출 |
| 13   | SignalRaised -> 다른 이벤트 종류             | ipc::event::tests::event_contract_group_18                                                                     | 검출 |
| 14   | SignalCleared -> 다른 이벤트 종류            | ipc::event::tests::event_contract_group_21                                                                     | 검출 |
| 15   | EstimateSubmitted -> 다른 이벤트 종류        | ipc::event::tests::event_contract_group_21                                                                     | 검출 |
| 16   | EstimateResolved -> 다른 이벤트 종류         | ipc::event::tests::event_contract_group_21                                                                     | 검출 |
| 17   | ValuationSnapshotTaken -> 다른 이벤트 종류   | ipc::event::tests::event_contract_group_24, ipc::event::tests::account_events_never_serialize_the_account_key  | 검출 |
| 18   | OrderRejected -> 다른 이벤트 종류            | ipc::event::tests::event_contract_group_24                                                                     | 검출 |
| 19   | OrderFilled -> 다른 이벤트 종류              | ipc::event::tests::account_events_never_serialize_the_account_key, ipc::event::tests::event_contract_group_27  | 검출 |
| 20   | OrderCancelled -> 다른 이벤트 종류           | ipc::event::tests::event_contract_group_27, ipc::event::tests::account_events_never_serialize_the_account_key  | 검출 |
