# U-02 IPC 계약

- 상태: 초안

## 기준과 범위

최초 작업 기준은 520c8dfc0e5b2fdd9bcc69a1c32b48b0eff9c49a, 독립 검토 기준은 master 수신 후 afc9a3f9841832585bcc48fe122e4f36cd27b367이다. 검토 대상은 최초 구현 484a4f26b604e37905ddb1485171402efadf5535와 1회차 수정 f1ab0358f8a305ed608d9b1ec5395d39aa069876이다. 마지막 수신 대상으로 4df1fac4a5b67bb2e77a948e41d470dc56cd97da을 고정했다. 작업은 IPC 틀이며 실제 화면용 명령을 늘리지 않는다.

## 결정

기존 app_version 명령 하나에 닫힌 current 조회 DTO를 연결한다. Tauri 비동기 명령으로 인자를 Future가 소유하게 하여 기존 needless_pass_by_value·unnecessary_wraps expect 두 개를 제거했다. 별도 작업 큐나 태스크는 추가하지 않는다.

입력은 serde deny_unknown_fields로 추가 필드를 거부한다. specta skip_attr = "serde"는 해당 DTO의 serde 메타데이터만 생략한다. 필드 이름·태그를 바꾸지 않는 현재 DTO에서 Rust가 받는 모양과 TS 모양이 같다. 별도 AppVersionQuery의 current 이름은 그 타입의 Type 정의가 반영한다. 수동 Type 구현으로 필드 목록을 복제하는 것보다 단순하며 끌 수 없는 린트를 약화하지 않는다. 향후 rename/default/skip/tag/flatten을 도입할 때에는 생성 비교뿐 아니라 정상 입력과 실제 IPC 경로의 동등성을 검증해야 한다.

ErrorHandlingMode::Result를 유지한다. ErrorClass의 UserAction, Wait, Bug, NoData를 각각 user-fixable, retryable, bug, no-data로 옮긴다. 오류에는 kind만 있고 진단·대기 시간·원문·경로·계좌 키는 없다. TS 경계는 오류를 strict 스키마로 검증하고 프레임워크 문자열·알 수 없는 객체를 payload 없는 framework 분류로 줄인다.

UiEvent는 외부 태그 열거형이다. 무페이로드는 종류 문자열, 나머지는 종류를 키로 갖는 객체다. 여섯 계좌 이벤트의 키를 버리고 다른 ID는 문자열로 보존한다. 공시는 소스와 접수 번호를 분리하며 시장·소스·통화쌍은 닫힌 DTO 열거형으로 옮긴다. 변환은 DomainEvent를 소진하므로 새 변형을 추가하면 검토를 요구한다. domain에는 직렬화 의존성을 추가하지 않는다.

UiEvent와 딸린 DTO는 단방향 전송을 위해 Serialize·Type만 갖고 Deserialize는 갖지 않는다. 이벤트 등록과 발행에 역직렬화는 필요 없다. 제품 조립 setup의 첫 줄에서 등록한다. 테스트는 build_app을 Ready까지 실행한 뒤 발행된 정확한 wire payload를 관측하고 자원을 정리한다. 등록 호출을 제거한 손 변형이 컴파일된 뒤 EventRegistry 누락으로 검출됐다.

기존 i64/u64/i128/u128/isize/usize export 거부와 기본 Typescript exporter를 유지한다. 현재 금액·수량 DTO는 없고 향후 경계에는 문자열 십진수만 사용한다. 해당 계산은 domain이 소유한다.

H-13 생성기와 재생성 바이트 비교를 유지한다. 이벤트 때문에 달라진 고정 헤더·런타임 스냅샷은 실제 생성물로 갱신한다. runtime import는 기존 core와 이벤트용 event를 두 생성 파일에서만 허용하고 mocks는 테스트 사본에만 둔다. TSImportType의 참조는 별도 typeImports에서 webview·window를 두 생성 파일에만 한정한다. responses는 타입 참조도 허용하지 않는다. runtime import 범위를 이 타입 목록 때문에 넓히지 않는다. 정확한 경로·값·개수와 허용·거부 대조군을 검사한다.

Tauri의 바깥 invoke 인자 객체와 request DTO는 구분한다. 실제 제품 invoke handler를 호출하는 영구 테스트가 생성 바인딩의 정상 인자 {request:{query:"current"}} 성공과 request 내부 추가 필드의 serde 거부를 검증한다. 바깥 추가 키 무시는 독립 비판의 실제 실행으로 확인했으며 현재 입력 DTO 조건 위반으로 보지 않는다. 더 강한 O-08 승인 입력 조건은 별도로 검증한다.

## 승인된 기존 테스트 변경

| 파일                                   | 테스트                                                                       | 전 → 후                                                            |
| -------------------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| src-tauri/src/bindings/tests.rs        | generated_typescript_names_the_command_mocks_and_recorded_responses          | 인자 없음 → AppVersionRequest 서명                                 |
| src-tauri/src/ipc/tests.rs             | app_version_response_is_recorded_for_the_typescript_fake                     | handle만 전달 → current DTO와 비동기 block_on 호출. 응답 단언 유지 |
| src-tauri/src/ipc/tests.rs             | every_ipc_error_is_recorded_with_its_tag                                     | 세 분류 → NoData 포함 네 분류                                      |
| desktop/tests/generated-files.test.mjs | the generated file list holds exactly the IPC outputs of gen-bindings        | event import 및 타입 참조 metadata의 정확한 목록·개수 반영         |
| desktop/tests/generated-files.test.mjs | generated files may import only their listed modules and nothing dynamically | event가 추가된 정확한 오류 문구. 다른 위반 단언 유지               |

기존 테스트 변경은 고유 테스트 5개다. 최초 구현의 Rust use 구문 변경은 같은 서명 변경의 support-removed/shadowing 진단 두 개를 냈다. 새 패키지와 새 검사 끄기는 없다.

## 보호 파일 변경

| 파일                                                 | 이유                                                                                                      |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| .codex/config.toml, harness/claude-hooks/hashes.json | 새 ADR 보호 등록과 생성 해시. 최신 master 수신 충돌은 생성기로 해결                                       |
| harness/generated-files/files.json, fixed.json       | 이벤트 생성물의 runtime import·타입 참조·고정 헤더·런타임 등록                                            |
| apps/desktop/eslint/generated.mjs                    | 허용 목록 밖 TSImportType을 거부하는 검사 추가                                                            |
| harness/capability-check/webview-apis.toml           | 새 mock 제품 조립 테스트 한 파일에서만 이벤트 발행·관측과 InvokeRequest 키 필드 허용. 제품 권한 추가 없음 |
| apps/desktop/tests/generated-files.test.mjs          | 생성 metadata의 정확한 기대값 갱신과 새 허용·거부 회귀                                                    |

harness/capability-check/webview-api/cases.toml에는 invoke_key의 제품 파일 거부와 지정한 mock 테스트 파일 허용 대조군 두 개만 추가했다. 기존 사례의 단언과 기대값은 바꾸지 않았다.

## 검증과 인계

독립 Opus와 Codex 검토를 두 회차 거쳤다. 1회차 연결 테스트와 타입 참조 검사 공백을 보완했고 2회차에 현재 차단·중요 결함은 확인되지 않았다. 2회차의 사소한 실제 invoke 테스트 공백은 영구 회귀로 보완했다. 프로젝트 신뢰와 실제 세션 훅 신뢰는 확인 불가이며 Git 검사 결과와 동일시하지 않는다. 환경 예외와 최종 검사 결과는 docs/reviews/U-02.md에 구분해 기록한다.

| 작업           | 인계                                                                                                                  |
| -------------- | --------------------------------------------------------------------------------------------------------------------- |
| U-04           | no-data 빈 상태 처리, IPC 시간 제한과 쿼리 계층                                                                       |
| U-03/U-05      | 생성 TS emit/emitTo는 제품 ACL상 허용되지 않는다. mock 성공을 제품 발행·구독 승인 증거로 쓰지 않고 단방향 구독을 검증 |
| U-05/A-10      | 이벤트 발행·구독과 선택 계좌 재조회 연결. 반드시 등록 이후 발행하도록 수명 주기 소유권 유지                           |
| O-08           | proposal_id만 받는 승인 입력의 바깥·안쪽 추가 키를 실제 IPC로 검증. serde 속성 확장 시 Rust·TS 모양 동등성 확인       |
| U-04/U-09/U-13 | 금액·수량 DTO를 도입할 때 십진수 문자열 경계와 표시 스키마 검증                                                       |

## 손 변형

| 번호 | 바꾼 것                                      | 잡은 테스트                                                                                                    | 결과                                     |
| ---- | -------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| 1    | PortfolioSyncRecorded -> 다른 이벤트 종류    | ipc::event::tests::event_contract_group_0, ipc::event::tests::account_events_never_serialize_the_account_key   | 검출                                     |
| 2    | TransactionsSyncRecorded -> 다른 이벤트 종류 | ipc::event::tests::event_contract_group_0, ipc::event::tests::account_events_never_serialize_the_account_key   | 검출                                     |
| 3    | PriceUpdated -> 다른 이벤트 종류             | ipc::event::tests::event_contract_group_0, ipc::event::tests::non_account_identifiers_keep_their_exact_strings | 검출                                     |
| 4    | PriceMoved -> 다른 이벤트 종류               | ipc::event::tests::event_contract_group_3                                                                      | 검출                                     |
| 5    | OrderSubmitted -> 다른 이벤트 종류           | ipc::event::tests::non_account_identifiers_keep_their_exact_strings, ipc::event::tests::event_contract_group_9 | 검출                                     |
| 6    | BrokerConnectionChanged -> 다른 이벤트 종류  | ipc::event::tests::event_contract_group_12                                                                     | 검출                                     |
| 7    | SettingsChanged -> 다른 이벤트 종류          | ipc::event::tests::event_contract_group_12                                                                     | 검출                                     |
| 8    | PolicyChanged -> 다른 이벤트 종류            | ipc::event::tests::event_contract_group_12                                                                     | 검출                                     |
| 9    | OrderSwitchChanged -> 다른 이벤트 종류       | ipc::event::tests::event_contract_group_15                                                                     | 검출                                     |
| 10   | WatchlistChanged -> 다른 이벤트 종류         | ipc::event::tests::event_contract_group_15                                                                     | 검출                                     |
| 11   | ThesisUpdated -> 다른 이벤트 종류            | ipc::event::tests::event_contract_group_15                                                                     | 검출                                     |
| 12   | SelfCheckChanged -> 다른 이벤트 종류         | ipc::event::tests::event_contract_group_18                                                                     | 검출                                     |
| 13   | SignalRaised -> 다른 이벤트 종류             | ipc::event::tests::event_contract_group_18                                                                     | 검출                                     |
| 14   | SignalCleared -> 다른 이벤트 종류            | ipc::event::tests::event_contract_group_21                                                                     | 검출                                     |
| 15   | EstimateSubmitted -> 다른 이벤트 종류        | ipc::event::tests::event_contract_group_21                                                                     | 검출                                     |
| 16   | EstimateResolved -> 다른 이벤트 종류         | ipc::event::tests::event_contract_group_21                                                                     | 검출                                     |
| 17   | ValuationSnapshotTaken -> 다른 이벤트 종류   | ipc::event::tests::event_contract_group_24, ipc::event::tests::account_events_never_serialize_the_account_key  | 검출                                     |
| 18   | OrderRejected -> 다른 이벤트 종류            | ipc::event::tests::event_contract_group_24                                                                     | 검출                                     |
| 19   | OrderFilled -> 다른 이벤트 종류              | ipc::event::tests::account_events_never_serialize_the_account_key, ipc::event::tests::event_contract_group_27  | 검출                                     |
| 20   | OrderCancelled -> 다른 이벤트 종류           | ipc::event::tests::event_contract_group_27, ipc::event::tests::account_events_never_serialize_the_account_key  | 검출                                     |
| 21   | 제품 조립의 mount_events 호출 제거           | tests::ipc_contract::product_assembly_mounts_the_event_and_emits_only_its_wire_payload                         | 컴파일 후 EventRegistry 누락 실패로 검출 |
