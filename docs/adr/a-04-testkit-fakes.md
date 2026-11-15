# A-04 포트 가짜와 고정 시계

- 상태: 초안

## 결정

기준은 580a19874ee4ad32c21c4c3f83a0a4142b3a4858이다. testkit은 기존 속성 테스트 도우미만 가진 잎으로 두며 작업 공간 crate에 의존하지 않는다. 포트 가짜와 공개 계약 검사는 testkit-ports로 분리하고 application과 domain에만 일반 의존한다. 새 외부 패키지·비동기 런타임·기능·실제 API 호출은 추가하지 않는다. 기존 property_cases 도우미는 그대로 둔다.

메인 결정에 따라 세션 훅 미적용을 중단 사유로 사용하지 않는다. 세션 훅 상태는 **미적용 확인됨(차단돼야 할 `git -c core.hooksPath=/tmp status`가 실행됨)**이다. 프로젝트 신뢰와 훅 정의 신뢰의 저장 상태나 미적용 원인은 단정하지 않는다. 정상 Git 훅을 실행하며 보호 규칙 무단 변경, master 이동, push, PR, 홈 설정 변경을 하지 않는다.

## A-04에서 정정: 주문 요약 참조

메인이 ADR 0045의 요약 참조 전용 경계를 우선하도록 결정했다. UserPresence::confirm_order는 &OrderSummary를 받는다. A-02의 값 인자와 architecture 3.3의 계약 표를 정정한다. 확정 ADR 0080의 원문은 고치지 않는다. 기존 ports_a02_contract.rs의 system_traits_match_the_contract_and_have_no_extra_methods는 기대 서명의 인자에 & 하나만 추가한다. 나머지 단언을 유지한다. 이는 메인이 승인한 서명 변경에 따른 기계적 변경이다.

confirm_risk_change의 RiskChange와 다른 포트의 값 인자를 점검했다. RiskChange는 공개 닫힌 변형으로 생성하고 복제할 수 있으며, 커서·시각·통화·시장·SecretKey 등도 공개 값이다. SecretStore::store의 SecretValue는 공개 생성 후 소유권을 이전한다. 같은 형태의 값 인자 정정은 추가로 필요하지 않았다.

OrderSummary에 Clone을 추가하지 않고 into_execution_parts의 가시성도 유지한다. RecordedOrderSummary는 공개 접근자로 읽는 열 가지 사실을 보관한다: 종목명, 코드, 시장, 방향, 수량, 지정가, 예상 금액(금액·기준·원화 환산 사실 포함), 예상 수수료, 마스킹한 계좌 뒤 네 자리, 주문 키 접두사. 원래 전체 계좌나 전체 주문 키는 요약의 공개 사실이 아니므로 기록에 발명하지 않는다. 기록은 승인 토큰이 아니다.

## 대본과 호출 기록

Script는 메서드별 FIFO 응답과 받은 인자를 보관한다. push로 다음 응답을 더하고 run이 호출될 때 한 개를 소비한다. 새 대본의 기본 상태는 비어 있다. 소진은 첫 poll에서 설명 있는 단언 실패다. 반환값 기본값이나 성공 응답을 합성하지 않는다. 표준 Future·Waker로 저장소의 기존 포트 테스트와 같은 한 번의 poll을 사용한다. IO·타이머·스레드·비동기 런타임을 만들지 않는다.

각 가짜는 메서드별 인자 기록과 포트 전체의 닫힌 호출 열거형 기록을 함께 제공한다. 공개 script_<메서드>() 접근자로 대본을 설정하고 calls()로 순서를 검사한다. 기록은 호출 Future가 실행될 때 남는다. 독립된 동시 호출 사이의 순서는 실제 기록 잠금을 얻은 순서이며 업무적인 순서를 만들지 않는다.

ScriptedPresence는 주문과 위험 변경에 서로 다른 결과 큐를 사용한다. 항상 Confirmed인 편의 생성자는 없다. SteppedClock은 명시적 advance_to 때만 시각을 바꾼다. 역행은 Backwards, 같은 시각은 성공이다. now는 동기이며 조회한 시각을 기록한다.

## 포트 대응

| 가짜 | 포트 |
|---|---|
| ScriptedBrokerRead, ScriptedMarketInfo | ADR 0055·0082·0087의 읽기 메서드 29개 |
| BrokerCapabilities 값 | 기존 닫힌 지원 표를 직접 사용. 별도 IO 가짜가 필요하지 않음 |
| MemoryPortfolioRepository 등 저장소 10개 | 메모리 저장·조회와 메서드별 대본 |
| MemoryAuditLog | 감사 추가와 검증 대본 |
| MemorySecretStore | 메모리 비밀 저장·조회·삭제와 오류 대본 |
| ScriptedNotifier | 알림 결과와 호출 기록 |
| ScriptedNewsFeed, ScriptedFundamentals, ScriptedMacroSeries | 커서·정책·데이터 부재·오류를 보존하는 대본 |
| SteppedClock, ScriptedPresence | 동기 시계와 종류별 확인 대본 |

AiProvider와 가짜 CLI는 만들지 않는다(A-11). NewsFeed의 source·metadata는 생성 시 고정하는 소스 식별·정책이며 호출 순서는 기록한다. 수집 결과는 new_items의 대본으로 설정한다.

## 메모리 저장소

MemoryReply::Memory를 명시적으로 넣으면 포트 계약의 저장·조회 동작을 수행하고, Return(result)는 지정 결과만 반환한다. Return 오류는 저장 상태를 바꾸지 않는다. 대본을 넣지 않은 메서드는 실패하므로 조용한 메모리 기본 동작이 없다. 동기화 기록은 계좌별 recorded_at 최신, 시세는 종목별 collected_at 최신, 환율은 통화쌍·종류와 지정 시각 이하 최신을 조회한다. 봉과 평가 이력은 [start,end) 범위로 과거순 반환한다.

추정 제출·감사 기록·평가 기록의 동일 재요청은 AlreadyRecorded다. 같은 과제의 판정은 최초 값을 보존한다. 보정 자료는 동일 키·동일 자료를 중복 추가하지 않고 새 버전은 추가하며 최신 버전을 조회한다. 빈 신호 목록도 현재 상태로 저장한다. 확인·방향·금액·가격·기간 계산은 하지 않는다.

Thesis는 공개 Clone이 없으므로 HistoryEntry 사실만 저장하고 ADR 0030에 따라 domain의 공개 적용·확인·거절 API로 복원한다. 가짜가 논거 규칙을 구현하지 않는다. Proposal<Draft>는 빌린 저장 입력을 소유한 초안 조회로 보존하기 위해 Draft에만 한정한 Clone impl을 domain에 추가한다. 비공개 range를 포함해 복사하며 새 회귀가 모든 사실과 재검증 보고의 동등성을 확인한다. Validated·ExecutionToken·OrderSummary·PendingOrderConfirmation의 복제나 생성 경계를 넓히지 않는다.

NewsRepository의 저장 입력 NewsItem에는 종목 연결 정보가 없다. 메모리 가짜는 기사 제목·본문·URL에서 종목을 추측하지 않는다. link는 종목 연결 메타데이터만 등록한다. for_instrument는 그 연결 중 store가 성공한 기사만 반환한다. 저장 전이나 실패한 저장은 조회에 나타나지 않는다. 동일 기사 중복 저장과 연결은 결과를 늘리지 않는다. 실제 저장소의 종목 연결 생성·보관 계약은 S-05 인계다.

SecretValue는 기존 application 타입을 사용한다. 호출 기록은 Arc로 소유권을 보존하며 Debug는 기존 비밀 가림을 따른다. load는 가짜 메모리 경계에서 노출된 바이트를 새 SecretValue로 감싼다. 실제 키나 계좌 자료를 넣지 않는다.

## 공개 계약 검사

contracts 모듈은 포트를 제네릭으로 받고 ADR의 입력·성공 값·부재·오류를 독립된 expected 사실과 대조하는 메서드별 함수를 공개한다. broker_read_*, market_info_*, 저장소 10개의 메서드, audit_log_*, secret_store_*, notifier_notify, news_feed_*, fundamentals_statements, macro_series_daily_values, clock_now, user_presence_order, user_presence_risk_change와 broker_capabilities가 있다. 어댑터는 녹화/고정 fixture에서 기대값을 만들고 같은 함수에 자기 구현을 전달한다. 가짜 출력에서 기대값을 추출하지 않는다.

읽기 포트 29개는 오류와 실제 요청 인자 기록을 각각 검사한다. 중복 시세 슬롯의 성공·실패 보존도 검사한다. 시계·대본·메모리 상태·뉴스 정책·부재 구분·감사 중복·비밀 키 격리를 별도로 검사한다. 포트 trait 정의나 가짜 대본 자체가 실제 HTTP의 페이지 완전성·배치 제한·원본 진위를 강제한다고 주장하지 않는다.

현재 domain의 공개 validate는 아직 외부 게이트를 Unknown으로 남긴다. 실제 PendingOrderConfirmation을 생산하는 주문 유스케이스 전에는 공개 소비자가 주문 요약을 만드는 정상 성공 fixture를 얻을 수 없다. 주문 결과 타입의 세 응답 FIFO는 공통 Script에서 실행하고, 주문 참조 계약 함수·열 필드 기록 변환과 가짜 trait impl은 컴파일로 확인한다. 실제 주문 요약을 통한 확인 호출 연동은 O-01·O-17에서 같은 공개 계약 함수로 검사한다. 이 한계를 숨기거나 내부 토큰 생성자를 공개하지 않는다.

## 보호 등록과 검사

새 crates/testkit-ports/src/scripted_presence.rs를 agent-writable의 보호 예외 경로로 등록하고 edit-cases에 잠금 전 경고·잠금 후 거부의 두 fixture를 추가한다. 기존 하네스 테스트 단언은 바꾸지 않는다. Codex 프로필·샌드박스 조각·Claude 훅 해시는 저장소의 공식 생성 명령으로 갱신한다. 규칙을 삭제하거나 다른 제품 코드를 허용하는 변경은 하지 않는다. 기존 arch-check의 testkit 일반 의존 제한·제품 바이너리 금지·order-execution 전이 금지 규칙을 유지한다.

시작 frozen install과 lefthook install은 성공했다. core 테스트는 공개 API·의존성이 없는 상태에서 컴파일 실패를 먼저 확인했다. 실행한 세부 검사는 리뷰 기록에 적는다. 실제 배포·계좌·본인 확인·주문 전송을 검증했다고 주장하지 않는다.

## 인계

| 받는 작업 | 내용 |
|---|---|
| A-06~A-17 | 명시적인 메서드별 대본과 MemoryReply로 성공·일부 실패·부재·거절 경로 구성 |
| A-11 | AiProvider 및 가짜 CLI 실행 파일. 이번 작업에는 없음 |
| T·S 묶음 | 녹화·메모리 fixture의 기대 사실로 공개 contracts 함수를 실제 어댑터에 실행 |
| S-05 | 기사와 종목의 연결 생성·저장 입력 계약 확정 |
| S-10 | 투자 성향과 위험 설정의 단일 저장 사실·트랜잭션 계약을 실제 저장소에서 강제 |
| O-01·O-17 | 정상 주문 게이트가 생산한 PendingOrderConfirmation의 요약 참조를 이용해 확인·기록 계약 연동 |

## 손 변형

별도 mktemp 복사본에서 컴파일과 테스트를 각각 실행했다. 최초 실행은 24개 검출·1개 생존이었다. 생존은 메모리 시세 조회의 종목 필터 반전이며, 서로 다른 두 종목의 시세를 저장·조회하는 회귀를 추가했다. 해당 회귀는 변형 상태에서 실패, 복구 상태에서 통과했다. 이후 새 복사본에서 25개 전부를 다시 실행하여 컴파일 성공 25·검출 25·생존 0·컴파일 불가 0을 확인했고, 전체 복구 후 기준 테스트도 통과했다. 최초 생존을 숨기거나 컴파일 불가를 검출로 세지 않는다.

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|

| 1 | 시계 동일 시각 거부 | 컴파일 성공·검출 | tests::clock_never_moves_on_its_own_and_rejects_backwards |
| 2 | 시계 비교 방향 반전 | 컴파일 성공·검출 | tests::clock_never_moves_on_its_own_and_rejects_backwards |
| 3 | 시계 모든 이동 거부 | 컴파일 성공·검출 | tests::clock_never_moves_on_its_own_and_rejects_backwards |
| 4 | 시계 동일 시각만 거부 | 컴파일 성공·검출 | tests::clock_never_moves_on_its_own_and_rejects_backwards |
| 5 | 시계 전진 누락 | 컴파일 성공·검출 | tests::clock_never_moves_on_its_own_and_rejects_backwards |
| 6 | 역행 성공으로 보고 | 컴파일 성공·검출 | tests::clock_never_moves_on_its_own_and_rejects_backwards |
| 7 | 응답 큐 역순 | 컴파일 성공·검출 | tests::risk_confirmation_is_scripted_and_recorded_separately, tests::scripts_preserve_order_and_arguments, tests::order_results_and_later_script_pushes_preserve_fifo |
| 8 | 추가 응답 앞에 삽입 | 컴파일 성공·검출 | tests::order_results_and_later_script_pushes_preserve_fifo |
| 9 | 호출 순서 반전 | 컴파일 성공·검출 | tests::risk_confirmation_is_scripted_and_recorded_separately, tests::scripts_preserve_order_and_arguments |
| 10 | 호출 조회 역순 | 컴파일 성공·검출 | tests::scripts_preserve_order_and_arguments, tests::risk_confirmation_is_scripted_and_recorded_separately |
| 11 | 대본 소진 조건 반전 | 컴파일 성공·검출 | tests::order_results_and_later_script_pushes_preserve_fifo, tests::scripts_preserve_order_and_arguments, tests::risk_confirmation_is_scripted_and_recorded_separately |
| 12 | 이전 호출 기록 삭제 | 컴파일 성공·검출 | tests::risk_confirmation_is_scripted_and_recorded_separately, tests::scripts_preserve_order_and_arguments |
| 13 | 최신 동기화 대신 과거 기록 | 컴파일 성공·검출 | tests::latest_sync_preserves_accounts_time_and_failures |
| 14 | 동기화 계좌 결합 반전 | 컴파일 성공·검출 | tests::latest_sync_preserves_accounts_time_and_failures |
| 15 | 환율 통화쌍 결합 반전 | 컴파일 성공·검출 | tests::fx_selects_pair_kind_and_latest_not_after_requested_time |
| 16 | 환율 종류 결합 반전 | 컴파일 성공·검출 | tests::fx_selects_pair_kind_and_latest_not_after_requested_time |
| 17 | 환율 정확한 시각 제외 | 컴파일 성공·검출 | tests::fx_selects_pair_kind_and_latest_not_after_requested_time |
| 18 | 환율 미래 자료 선택 | 컴파일 성공·검출 | tests::fx_selects_pair_kind_and_latest_not_after_requested_time |
| 19 | 환율 최신 대신 과거 | 컴파일 성공·검출 | tests::fx_selects_pair_kind_and_latest_not_after_requested_time |
| 20 | 시세 종목 결합 반전 | 컴파일 성공·검출 | tests::quote_memory_never_returns_another_instruments_latest_fact |
| 21 | 감사 중복 판정 반전 | 컴파일 성공·검출 | tests::audit_is_append_only_idempotent_and_verify_is_scripted |
| 22 | 감사 추가 순서 반전 | 컴파일 성공·검출 | tests::audit_is_append_only_idempotent_and_verify_is_scripted |
| 23 | 비밀 키 조회 반전 | 컴파일 성공·검출 | tests::secret_memory_round_trip_replacement_removal_and_key_isolation |
| 24 | 비밀 교체 보존 조건 반전 | 컴파일 성공·검출 | tests::secret_memory_round_trip_replacement_removal_and_key_isolation |
| 25 | 비밀 삭제 보존 조건 반전 | 컴파일 성공·검출 | tests::secret_memory_round_trip_replacement_removal_and_key_isolation |

## 정상 커밋 훅의 차단: 고정 cargo-deny의 순환 재귀

첫 정상 훅은 새 보호 경로에 대한 edit-case 누락을 잡았다. 기존 단언은 유지하고 잠금 전 경고·잠금 후 거부 fixture 두 개를 추가하여 edit-guard 7개가 통과했다. ScriptedPresence의 해시 대상은 hashes.mjs의 NAMED에 이 새 파일 하나만 추가하고, 실제 파일의 SHA-256이 등록되는 새 회귀로 확인한다.

두 번째 정상 훅은 check-fast에서 전체 검사로 승격하여 진행한 뒤 cargo deny 0.20.2에서 stack overflow, SIGABRT로 종료했다. 단순 진단 출력 문제로 가정하고 숨김·JSON·최단 경로 표시를 실험했지만 모두 같은 종료를 보였다. 출력 옵션 실험의 justfile 변경은 회수했다. deny.toml과 도구 버전은 바꾸지 않았다.

설치된 고정 도구 소스의 bans.rs:1179 needs_checking은 direct_dependents를 방문 집합 없이 재귀한다. 현재 정책은 include-dependencies=true, include-workspace=true이며, testkit의 일반 의존과 domain/application의 기존 개발 의존이 만드는 합법적인 순환에서 이 재귀가 끝나지 않는다.

외부 패키지가 없는 두 crate 최소 재현에서도 같은 정책으로 cargo deny --locked --offline check --hide-inclusion-graph가 SIGABRT(-6)였다. 최소 재현에서 개발 의존 간선 하나만 제거하고 같은 정책으로 실행하면 종료 0이며 advisories ok, bans ok, licenses ok, sources ok였다. 재현 파일과 실제 로그는 target/A-04/deny-cycle 및 같은 디렉터리의 deny-cycle-fail.log·deny-cycle-control.log에 보존한다. 제품 저장소의 개발 의존을 제거한 것은 아니다.

개발 의존 제외, 빌드 전이 의존 검사 제외, 경고 허용, 의미 없는 build.rs 추가, 설치된 검사 바이너리 교체로 통과시키지 않는다. 정상 훅의 다른 실패가 남았으므로, 승인된 test-diff 변경만 있을 때의 LEFTHOOK=0 예외도 사용하지 않는다. 검사 범위를 유지하는 고정 도구의 순환 재귀 수정이 선행되어야 한다.

master와의 분기 기준 580a19874ee4ad32c21c4c3f83a0a4142b3a4858에 대해 test-diff-selected를 실제 실행한 결과는 승인된 A-02 서명 기대 문자열 변경 1건뿐이었다. 최신 master 735ea38189edbf15d34238dd14ac49bb02c9e94f 자체는 아직 HEAD의 조상이 아니어서 해당 끝을 직접 기준으로 한 실행은 조상 검사에서 중단됐다. 최신 master 병합 후의 판정과 혼동하지 않는다.

이 절은 구조 분리 승인 전의 실패 기록이다. 이후 결정으로 이 도구 오류를 해결했고 구현 2a3a376과 최신 master e2b9405 병합 091f01a를 만들었다. 정상 훅은 전체 검사와 커버리지를 통과한 뒤 승인된 test-diff 두 건에서만 중단했다. 나머지 후속 검사까지 확인한 뒤 브리프가 승인한 해당 커밋 한정 예외를 커밋 메시지에 기록했다.

변경분 자동 변이 명령 just mutants-diff 580a19874ee4ad32c21c4c3f83a0a4142b3a4858도 실제 실행했다. 사전 test-diff가 승인된 서명 기대 문자열 변경 1건을 거부해 변이 실행 전에 종료 1이었다. 변이 검사 통과나 변이 개수 0으로 기록하지 않는다. 최종 손 변형 대상 소스 4개의 SHA-256은 복구한 실험 복사본과 현재 작업 사본에서 일치했다.

## 메인 결정에 따른 해결: 3번 crate 구조 분리

2026-11-16 공식 GitHub releases API를 직접 확인했다. 최신 비초안·비사전 릴리스는 0.20.2이며 그보다 새 정식 버전이 없어 1번은 선택할 수 없었다. 조회한 태그·게시 시각·공식 URL은 target/A-04/cargo-deny-releases.json에 보존했다. 공식 릴리스: https://github.com/EmbarkStudios/cargo-deny/releases/tag/0.20.2.

2번은 검사 범위를 유지하는 설정으로 해결하지 못했다. needs_checking 재귀를 타게 하는 include-dependencies=true와 include-workspace=true는 현재 검사 범위의 일부다. 출력 숨김·JSON·최단 경로 강조 변경은 모두 같은 최소 재현에서 실패했다. 개발 의존이나 전이 의존·작업 공간을 제외하지 않았고, executables/interpreted를 허용으로 바꾸지도 않았다.

처음으로 검사를 보존하며 성공한 3번을 선택한다. testkit의 기존 lib.rs와 Cargo.toml은 기준 상태로 복구했다. 새 testkit-ports가 포트 가짜와 계약 검사·그 테스트를 소유하며 application/domain에 의존한다. application/domain은 testkit-ports에 개발 의존하지 않는다. 이 금지를 dev-edge와 전이 금지 규칙으로 고정하고, 제품 bin 포함 금지 및 order-execution 전이 금지도 새 crate에 적용한다. UserPresence 구현 허용 위치를 platform-macos와 testkit-ports로 옮기고 testkit에는 허용하지 않는다.

동일한 deny.toml의 세 crate 최소 재현에서 domain의 개발 의존은 잎 testkit으로, 포트 가짜의 일반 의존은 별도 testkit-ports에서 domain으로 향하게 했다. cargo deny --locked --offline check는 종료 0이고 advisories/bans/licenses/sources 네 검사 모두 통과했다. 실제 작업 공간에서도 같은 명령과 원래 설정으로 네 검사 모두 통과했다. 도구 버전·바이너리·검사 범위는 변경하지 않았다.

architecture와 tasks의 A-06 이후 행에 가짜를 쓰는 유스케이스 테스트를 testkit-ports/tests/ 또는 별도 테스트 crate에 둔다는 인계를 적었다. 구현 코드는 application에 남긴다. 기존 arch-check fixture의 testkit 포트 의존을 제거하고 본인 확인 fixture를 testkit-ports로 이름만 옮겼다. 별칭 금지 반례의 기대 경로도 새 정확한 경로로 정정했으며 반례와 단언은 유지했다. 도우미 소스의 전체 검사 승격도 새 crate에 확장하고 회귀로 고정한다.

이 결정이 앞 절의 cargo-deny 차단 상태를 해결한다. 아직 독립 리뷰를 실행하기 전이므로, 구현 커밋 이후 새 Opus 리뷰와 Codex 비판을 수행한다. 최신 master를 병합할 때 A-21이 추가한 MarketInfo 메서드가 있으면 가짜·호출 기록·공개 계약 검사까지 함께 추가한다.

구조 분리 뒤 정상 훅의 전체 nextest는 1368개 중1367개 통과, workspace_members_match_architecture_layout 한 건이 새crate를 기존 고정목록에서 찾지 못해 실패했다. 기존 ARCHITECTURE_CRATES의22개 항목과 assert_eq는 유지하고 testkit-ports 항목 하나와 정확한 길이23을 추가했다. 이는 승인된 구조 변경에 따른 고정목록 갱신이다.

## 독립 리뷰 1 이후 보강

Opus 5.5와 별도 Codex gpt-6.1-sol이 계약 시나리오 부족과 테스트 누락을 재현했다. 메서드별 비교 함수에 더해 contracts::scenarios에 저장·조회 순서를 가진 제네릭 시나리오를 공개한다. 포트만 받아 동작하며 메모리 가짜에 의존하지 않는다. 서로 다른 계좌·종목·버전, 비어 있지 않은 신호, 가격 범위가 다른 같은 키의 초안, 확인·거절 이력을 증인으로 쓴다. 정상 기대값은 원본 fixture에서 가져오며 가짜의 복원 결과로 만들지 않는다. 쓰기를 버리는 신호 구현과 가격 범위를 바꾼 조회 결과를 거부하는 테스트를 실제 실행한다.

DraftRecord는 가격 범위까지 기록한다. Proposal의 안전한 가격 범위 읽기 접근자를 추가했다. Draft 한정 Clone은 빌린 초안 저장 계약을 소유한 조회로 보존하는 구현상 필요이며 검증 권한은 복사하지 않는다. OrderSummary·Validated·토큰의 복제나 생성 경계는 유지한다.

같은 봉의 일괄·분할 저장 결과가 달라지지 않도록 완전히 동일한 봉은 입력 묶음 안에서도 한 번만 저장한다. 값이 다른 수정 봉의 키별 병합 정책은 정하지 않고 S-03에 넘긴다. 별도 동시 호출 사이에 업무적인 순서를 만들지 않으며 순서에 의존하는 대본은 테스트가 호출을 직렬화한다.

투자 성향과 위험 설정은 공유하는 비중·위험 성향·시간 허용 사실을 함께 반영한다. domain의 저장 상태 복원 API로 검증하며 복원이 거부되는 변경은 InvalidData로 반환하고 상태를 보존한다. 가짜가 계산이나 사용자 승인 규칙을 다시 구현하지 않는다.

본인 확인 impl의 testkit-ports 허용은 보호된 scripted_presence.rs 한 파일로 한정한다. 공통 Script 엔진과 lib 모듈 연결도 보호 등록·해시에 포함한다. 다른 파일의 impl을 거부하는 음성 사례와 보호 파일의 정상 구현 양성 사례를 함께 검사한다. open-issues I-14의 경로는 사용자가 승인한 testkit-ports 구조와 기계적으로 맞춘다.

주문 확인의 정상 실행 fixture는 여전히 미해결이다. 공개 validate는 외부 게이트를 Unknown으로 남기므로 외부 테스트에서 토큰과 PendingOrderConfirmation을 만들 수 없다. 단독 요약 생성이나 토큰 공개는 사용자 금지 경계이며 추가하지 않는다. 주문 결과 FIFO의 공통 대본 시험과 컴파일 확인을 실제 confirm_order 실행으로 세지 않는다. 독립 리뷰 1의 지적 7은 남은 차단이다.

## 리뷰 반례의 손 변형 재검증

새 임시 복사본에서 기존 25개와 독립 비판자가 생존시킨 9개를 합쳐 34개를 각각 컴파일하고 core·memory·review_regressions 테스트로 실행했다. 컴파일 불가 0·검출 34·생존 0이며 원복 후 기준 테스트도 통과했다. 아래 아홉 개는 최초 25개의 검출에 섞어 생존이 없었던 것처럼 보고하지 않는다. 비판 당시에는 기존 memory 테스트가 놓쳤고 새 계약 시나리오가 검출했다.

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|
| 26 | 논거 최신 순서 누락 | 컴파일 성공·검출 | tests::generic_memory_scenarios_preserve_state_and_boundaries |
| 27 | 보정 최신 순서 누락 | 컴파일 성공·검출 | tests::generic_estimates_preserve_first_resolution_and_latest_calibration |
| 28 | 신호 최신 순서 누락 | 컴파일 성공·검출 | tests::generic_signal_contract_requires_nonempty_replacement_and_clearing |
| 29 | 제안 최신 순서 누락 | 컴파일 성공·검출 | tests::generic_memory_scenarios_preserve_state_and_boundaries |
| 30 | 최초 판정 중복 조건 반전 | 컴파일 성공·검출 | tests::generic_estimates_preserve_first_resolution_and_latest_calibration |
| 31 | 봉 종료 포함 | 컴파일 성공·검출 | tests::generic_memory_scenarios_preserve_state_and_boundaries |
| 32 | 평가 이력 종료 포함 | 컴파일 성공·검출 | tests::generic_memory_scenarios_preserve_state_and_boundaries |
| 33 | 논거 재생 삭제 | 컴파일 성공·검출 | tests::generic_memory_scenarios_preserve_state_and_boundaries |
| 34 | 논거 확인을 거절로 변경 | 컴파일 성공·검출 | tests::generic_memory_scenarios_preserve_state_and_boundaries |

## 최신 master A-21 통합

master dc8f3f950294e9d3d1947b39c256868076d7de15의 MarketInfo 다섯 메서드를 반영한다: investor_trading_rankings, sectors, sector, sector_rankings, sector_etfs. ScriptedMarketInfo는 모두 합쳐 14개이며 BrokerRead 15개와 합쳐 읽기 메서드는 29개다. 각 추가 메서드는 독립 응답 큐·포트 전체 호출 열거형·메서드별 입력 기록·공개 제네릭 계약 함수를 제공한다. 빈 성공과 오류를 바꾸지 않는다. 새 기능 다섯 개도 명시적 BrokerCapabilities 값으로 검사한다.

서로 다른 상한·시장·섹터 ID·기간·종류로 요청하고 오류 다음 성공을 순서대로 반환한다. 성공 fixture는 domain의 공개 응답 검증 생성자로 만들며 서버 시각이 없는 사실·nullable 등락률·ETF 순서·그룹별 rankedAt을 임의 계산으로 채우지 않는다. 실제 HTTP 식별·전체 계층 대조·신선도 의무는 ADR 0087의 T-09/T-10 인계다. 이 추가 통합에서도 주문 요약 생성·권한 경계는 바꾸지 않는다.

A-21의 다섯 전달 경로도 새 mktemp 복사본에서 각 변형을 컴파일·실행했다. 다섯 개 모두 검출, 원복 기준 통과, scripted.rs 해시 일치다. 앞선 34개와 별개로 합쳐 실제 실행한 손 변형은 39개다. 자동 변경분 변이는 승인 test-diff 사전 검사와 별개로 통과를 주장하지 않는다.

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|
| 35 | 투자자 랭킹 전체 기록 삭제 | 컴파일 성공·검출 | tests::investor_trading_rankings_preserves_success_error_arguments_and_order |
| 36 | 섹터 목록 전체 기록 삭제 | 컴파일 성공·검출 | tests::sectors_preserves_success_error_arguments_and_order |
| 37 | 섹터 상세 전달 시장 고정 | 컴파일 성공·검출 | tests::sector_preserves_success_error_arguments_and_order |
| 38 | 섹터 랭킹 질의 고정 | 컴파일 성공·검출 | tests::sector_rankings_preserves_success_error_arguments_and_order |
| 39 | 관련 ETF 전달 시장 고정 | 컴파일 성공·검출 | tests::sector_etfs_preserves_success_error_arguments_and_order |

## 대본 사용 예

아래는 비동기 테스트 안에서 원화 현금 조회만 실패시키는 예다. 실제 계좌·키를 사용하지 않는다. 테스트의 즉시 Future 구동 방식은 tests/core.rs와 같다.

```rust
let broker = ScriptedBrokerRead::default();
let account = AccountId::new(17);
let error = PortError::new(
    ReadEndpoint::BuyingPower,
    HttpStatus::Forbidden,
    PortErrorKind::AccessDenied(AccessDenial::Forbidden),
);
broker.script_buying_power().push(Err(error));
contracts::broker_read_buying_power(&broker, &account, Currency::Krw, Err(error)).await;
assert_eq!(broker.calls(), vec![BrokerReadCall::BuyingPower(account, Currency::Krw)]);
```

메모리 저장소는 script_<메서드>().push(MemoryReply::Memory)로 실제 저장 의미를 선택하거나 MemoryReply::Return(Err(error))로 한 호출의 오류를 선택한다. 등록하지 않은 호출은 즉시 실패한다. 계약 함수는 메서드별 74개와 저장 의미의 제네릭 시나리오 16개다. 주문용 user_presence_order의 실제 실행은 앞서 기록한 차단이며 다른 계약 실행으로 대체했다고 세지 않는다.

## 독립 리뷰 2 이후 보강과 미확정 계약

대상 1b7ae92의 새 Opus와 새 Codex 비판에서 초기 설정의 반대 조회 부재와 공개 시나리오의 키 격리 부족을 확인했다. 성향만 먼저 저장해도 위험 설정의 공유 사실을 만들고, 위험 설정만 먼저 저장해도 domain 기본 성향에 공유 사실을 반영한다. 최초 성향 저장에 아직 없던 주문·시장가 스위치는 Disabled로 채우며 기존 스위치는 보존한다. domain 복원 실패는 양쪽 상태를 바꾸기 전에 반환한다. 비기본 보유 기간·성향·비중·시간 허용을 서로 다른 값으로 검증하고, 후속 쓰기가 앞선 오류를 덮기 전에 즉시 읽는다. 위험 설정 선행 저장도 별도 공개 제네릭 시나리오로 검증한다.

공개 저장 시나리오에는 다른 제안 ID·과제 키와 종목·보정 키·뉴스 종목·같은 기간의 다른 계좌를 함께 저장한다. 봉은 같은 시간 범위에 다른 종목·주기·수정 기준 자료를, 환율은 다른 통화쌍·종류 자료를 함께 둔다. 주 조회뿐 아니라 보조 키의 사실도 확인해 키 무시를 검출한다. 봉 인자 종목과 자료 종목이 어긋나는 입력은 보수적으로 InvalidData로 거부하고 묶음 전체를 저장하지 않는다. 공개 계약은 구체 오류 종류 대신 거부와 부분 저장 없음을 요구한다. S-04는 실제 어댑터 오류 분류를 정합화한다.

완전 동일 봉의 멱등성은 정했지만 동일 OHLCV·봉 시각에서 collected_at만 다른 재수집, Provisional에서 Finalized로의 상태 변화, 값이 다른 수정 봉을 같은 키로 갱신·병합할지는 정하지 않았다. 현재 가짜는 완전 동일성 기준으로만 중복을 제거한다. 이 관측 메타데이터·상태까지 포함하는 키 정책은 S-03에 인계한다. 이력에서 재수집을 별개 관측으로 볼지 확정 전에는 키 갱신 규칙을 발명하지 않는다.

Hours 보유 기간에서 시간 허용을 해제하면 domain 복원이 거부한다. 가짜는 InvalidData를 반환하며 양쪽 기존 값을 보존한다. 자동 기간 변경 여부와 입력 충돌의 오류 분류는 S-10에 인계한다. 보정 자료 A→B→A는 같은 사실 A의 재요청으로 AlreadyRecorded이며 최신 B를 유지한다. 같은 값을 가진 새 논리 버전과 동일 기록 재요청을 구별할 식별 계약은 S-06에 인계한다.

Draft 한정 Clone과 가격 범위 읽기는 빌린 초안의 모든 사실을 보존하는 A-04 저장소 구현 범위다. 독립 비판자는 Draft 복제 성공과 Validated·ExecutionToken·OrderSummary·PendingOrderConfirmation 복제 실패, 비공개 생성·분해 실패를 실제 컴파일로 확인했다. 권한 확대 주장은 기각됐고 별도 명시 승인 이력이 없다는 관찰을 무승인 보호 변경으로 확대하지 않는다. 주문 확인의 정상 fixture 차단은 이 안전한 읽기·초안 복제와 별개로 유지한다.

## 수정 2의 손 변형

수정된 소스에서 기존 34개를 새 임시 복사본으로 다시 컴파일·실행하여 검출 34·생존 0을 확인했다. A-21 다섯 변형의 원복 scripted.rs도 현재 소스와 SHA-256이 같다. 아래 21개는 review_regressions의 공개 시나리오 실행만으로 검출했다. 세 집합의 서로 다른 실제 컴파일 성공 변형은 60개, 검출 60·생존 0이다. 각 집합의 기준·원복 테스트가 통과했고 변형 대상 소스 해시를 현재 소스와 대조했다.

52번의 첫 단순 삭제는 사용하지 않는 helper가 생겨 컴파일 불가였다. 그 시도는 개수에 넣지 않는다. 같은 helper 결과를 계산한 뒤 버려 위험 설정을 None으로 만드는 동등한 결함으로 다시 컴파일·실행하여 검출했다. 로그와 JSON은 target/A-04/isolation-mutations-final.json 및 isolation-retry-mutations.json에 있다. 자동 변경분 변이의 test-diff 사전 차단은 이 손 변형 결과와 별도로 보고한다.

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|
| 40 | 봉 종목 필터 삭제 | 컴파일 성공·검출 | tests::generic_candle_and_valuation_contracts_preserve_query_scope |
| 41 | 봉 주기 필터 삭제 | 컴파일 성공·검출 | tests::generic_candle_and_valuation_contracts_preserve_query_scope |
| 42 | 봉 수정 기준 필터 삭제 | 컴파일 성공·검출 | tests::generic_candle_and_valuation_contracts_preserve_query_scope |
| 43 | 제출 조회 종목 무시 | 컴파일 성공·검출 | tests::generic_estimates_preserve_first_resolution_and_latest_calibration |
| 44 | 판정 조회 과제 키 무시 | 컴파일 성공·검출 | tests::generic_estimates_preserve_first_resolution_and_latest_calibration |
| 45 | 보정 조회 키 무시 | 컴파일 성공·검출 | tests::generic_estimates_preserve_first_resolution_and_latest_calibration |
| 46 | 제안 조회 키 무시 | 컴파일 성공·검출 | tests::generic_proposal_contract_preserves_versions_and_keys |
| 47 | 평가 조회 계좌 무시 | 컴파일 성공·검출 | tests::generic_candle_and_valuation_contracts_preserve_query_scope |
| 48 | 뉴스 조회 종목 무시 | 컴파일 성공·검출 | tests::news_contract_preserves_instrument_isolation_and_saved_facts |
| 49 | 환율 통화쌍 무시 | 컴파일 성공·검출 | tests::market_data_latest_queries_have_reusable_contract_sequences |
| 50 | 환율 종류 무시 | 컴파일 성공·검출 | tests::market_data_latest_queries_have_reusable_contract_sequences |
| 51 | 성향 기본값 대체 | 컴파일 성공·검출 | tests::settings_queries_reflect_the_same_shared_facts |
| 52 | 성향 저장의 위험 설정 누락 | 컴파일 성공·검출 | tests::settings_queries_reflect_the_same_shared_facts |
| 53 | 위험 저장의 성향 누락 | 컴파일 성공·검출 | tests::settings_risk_first_populates_the_same_profile_facts, tests::settings_queries_reflect_the_same_shared_facts |
| 54 | 봉 인자 불일치 검증 삭제 | 컴파일 성공·검출 | tests::candle_store_rejects_mismatched_instrument_without_partial_write, tests::generic_candle_and_valuation_contracts_preserve_query_scope |
| 55 | 성향 저장 시 기존 주문 스위치 손실 | 컴파일 성공·검출 | tests::settings_queries_reflect_the_same_shared_facts |
| 56 | 성향 저장 시 기존 시장가 스위치 손실 | 컴파일 성공·검출 | tests::settings_queries_reflect_the_same_shared_facts |
| 57 | 위험 저장 시간 허용 사실 손실 | 컴파일 성공·검출 | tests::settings_queries_reflect_the_same_shared_facts, tests::settings_risk_first_populates_the_same_profile_facts |
| 58 | 위험 저장 종목 상한 사실 손실 | 컴파일 성공·검출 | tests::settings_risk_first_populates_the_same_profile_facts, tests::settings_queries_reflect_the_same_shared_facts |
| 59 | 위험 저장 주의 비중 사실 손실 | 컴파일 성공·검출 | tests::settings_risk_first_populates_the_same_profile_facts, tests::settings_queries_reflect_the_same_shared_facts |
| 60 | 위험 저장 섹터 상한 사실 손실 | 컴파일 성공·검출 | tests::settings_risk_first_populates_the_same_profile_facts, tests::settings_queries_reflect_the_same_shared_facts |
