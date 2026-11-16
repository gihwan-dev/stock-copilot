# U-01 조립 루트와 수명 주기

- 상태: 초안

## 결정

기준은 e86f6fb12d35afda07149a103b9c94b9493ddd16이다. 메인 결정에 따라 프로세스 안의 작은 구현과 시스템 시계는 실제로 조립하고, 외부 의존 포트는 닫힌 실패를 반환한다. SystemClock 구현은 platform-macos에 둔다. 원시 시각 읽기는 system_clock.rs에만 허용한다. 기존 main.rs의 벽시계 직접 읽기 허용을 제거하고 위반 고정 자료를 추가한다.

SystemClock은 UTC 0001-01-01 00:00:00부터 9999-12-31 23:59:59.999999999까지 표현하며 범위 밖은 가까운 끝으로 고정한다. 음수 유닉스 시각의 분수는 초를 내리고 나노초를 보완한다. 운영체제 벽시계 조정을 그대로 따르므로 단조성을 보장하지 않는다. 실제 계좌, 키, API를 쓰지 않는다.

읽기 포트와 저장소 포트는 impl Future를 반환하므로 dyn 객체로 바꾸지 않는다. PortTypes의 연관 타입으로 구체 구현을 한 묶음으로 선택하고 Ports<P>의 각 필드를 Arc<P::Port>로 보관한다. build_app<R: Runtime, P: PortTypes>은 제품과 mock 런타임에서 동일하다. PortTypes는 새로운 업무 포트가 아니라 21개 구체 타입의 조립 매개변수다.

BrokerTrade는 order-execution 전용 포트이며 src-tauri가 직접 보지 않는다. 주문 포트는 Ports에 노출하지 않는다. 승인 서비스의 유스케이스 통합은 후속 O 작업에서 approval-command를 통해 조립한다. 새 IPC 명령과 프런트 화면은 만들지 않는다.

## Ports

| 필드             | 타입                            | 분류                                                            | 교체 또는 인계               |
| ---------------- | ------------------------------- | --------------------------------------------------------------- | ---------------------------- |
| `broker_read`    | `Arc<P::BrokerRead>`            | 닫힌 자리 표시: 외부 의존                                       | T-02, T-05~T-15              |
| `market_info`    | `Arc<P::MarketInfo>`            | 닫힌 자리 표시: 외부 의존                                       | T-08~T-11, T-16, T-21        |
| `macro_series`   | `Arc<P::MacroSeries>`           | 닫힌 자리 표시: 외부 의존                                       | N 수집 작업                  |
| `news_feeds`     | `Vec<Arc<P::NewsFeed>>`         | 등록 목록: 현재 비어 있음; 메타데이터는 생성 입력을 그대로 보존 | N 수집 작업                  |
| `fundamentals`   | `Arc<P::Fundamentals>`          | 닫힌 자리 표시: 외부 의존                                       | N 수집 작업                  |
| `ai_provider`    | `Arc<P::AiProvider>`            | 닫힌 자리 표시: 외부 의존                                       | AI 러너 작업                 |
| `portfolios`     | `Arc<P::PortfolioRepository>`   | 닫힌 자리 표시: 외부 의존                                       | S-02                         |
| `transactions`   | `Arc<P::TransactionRepository>` | 닫힌 자리 표시: 외부 의존                                       | S-03                         |
| `market_data`    | `Arc<P::MarketDataRepository>`  | 닫힌 자리 표시: 외부 의존                                       | S-04                         |
| `news`           | `Arc<P::NewsRepository>`        | 닫힌 자리 표시: 외부 의존                                       | S-05                         |
| `theses`         | `Arc<P::ThesisRepository>`      | 닫힌 자리 표시: 외부 의존                                       | S-06                         |
| `estimates`      | `Arc<P::EstimateRepository>`    | 닫힌 자리 표시: 외부 의존                                       | S-18                         |
| `signals`        | `Arc<P::SignalRepository>`      | 닫힌 자리 표시: 외부 의존                                       | S-07                         |
| `proposals`      | `Arc<P::ProposalRepository>`    | 닫힌 자리 표시: 외부 의존                                       | S-07                         |
| `valuations`     | `Arc<P::ValuationRepository>`   | 닫힌 자리 표시: 외부 의존                                       | S-18                         |
| `settings`       | `Arc<P::SettingsRepository>`    | 닫힌 자리 표시: 외부 의존                                       | S-11                         |
| `audit`          | `Arc<P::AuditLog>`              | 닫힌 자리 표시: 외부 의존                                       | S-08, S-16                   |
| `secrets`        | `Arc<P::SecretStore>`           | 닫힌 자리 표시: 외부 의존                                       | S-12                         |
| `clock`          | `Arc<P::Clock>`                 | 실제: 시스템 시계                                               | 이 작업                      |
| `notifier`       | `Arc<P::Notifier>`              | 닫힌 자리 표시: 외부 의존                                       | U-14                         |
| `presence`       | `Arc<P::UserPresence>`          | 닫힌 자리 표시: Unavailable                                     | O-07, U-24                   |
| `events`         | `EventBus`                      | 실제: 프로세스 내부 구현                                        | U-02, J-01이 구독자 등록     |
| `saved_accounts` | `Vec<AccountId>`                | 조립 입력: 저장된 계좌 인덱스, 제품은 현재 비어 있음            | S-02가 저장 계좌 인덱스 제공 |
| `database`       | `Arc<DatabaseFactory>`          | 실제 S-01의 열기 함수를 main.rs에서 주입                        | S-15 마이그레이션 통합       |

BrokerRead와 MarketInfo는 요청 엔드포인트를 보존한 AccessDenied(InvalidCredentials), HttpStatus::NotReceived를 반환한다. 시도하지 않은 HTTP 상태를 만들지 않는다. 저장소·감사·알림·비밀 쓰기는 SystemFailure::Unavailable(RetryDelay::Unknown)이며 SecretStore::load만 Ok(None)으로 설정 부재를 보존한다. 선택 자료(MacroSeries, Fundamentals, NewsFeed)는 NotConfigured다. AI 실행과 스트림은 InformationError::Unavailable(NotProvided)다. UserPresence는 주문과 위험 변경 모두 Unavailable이며 확인 성공을 만들지 않는다.

NewsFeed의 source와 metadata는 오류 없는 값이므로 생성 입력을 그대로 보존한다. 실제 소스가 등록되기 전에는 피드를 지어내지 않고 빈 목록을 조립한다. 이벤트 버스는 EventBusBuilder로 실제 구현을 만든다.

## 시작

1. main.rs가 SystemClock, EventBus와 닫힌 자리 표시, Database::open 팩터리를 조립한다.
2. Tauri 창 설정을 숨김으로 고정한다. setup 전에 만들어지는 창도 표시되지 않는다.
3. SecretStore에서 DatabaseEncryptionKey를 읽는다. None이면 경로 조회·폴더 생성·Database::open 모두 호출하지 않고 NotConfigured를 보존한다. 오류도 그대로 보존한다.
4. 키가 있으면 앱 데이터 경로를 구하고 블로킹 작업에서 DB를 연다. 실제 Database 생성 표현식은 main.rs에만 있다.
5. 저장된 성향·위험 설정·계좌별 현황을 읽는다. 부재·저장 오류·부분 동기화 사실을 유지한다. 계좌 목록을 네트워크에서 먼저 가져오지 않는다.
6. MainScreenData를 앱 상태에 넣고 main 창을 표시한다. 표시 실패면 네트워크 갱신을 시작하지 않는다.
7. 그 뒤 BrokerRead::accounts 갱신을 비동기 작업으로 시작한다. Ports와 DB 상태 및 작업 핸들을 앱이 소유한다.

PERF-1의 순서 테스트는 메모리 저장소에 서로 다른 원화·달러 금액과 관측 시각을 넣고, 창 표시 콜백에서 저장 자료와 네트워크 호출 0회를 확인한다. Tauri mock 런타임 조립 테스트도 같은 build_app과 setup을 실행한다. 실제 주 화면 렌더링과 2초 성능 측정은 U-04, G-09에 남는다.

## 종료

run_app의 ExitRequested 처리에서 종료를 미루고 Lifecycle::shutdown을 비동기로 실행한다. refresh를 abort한 뒤 완료를 기다리고, 실제 Database 소유권은 spawn_blocking으로 옮겨 Database::shutdown을 호출한다. S-01 Writer는 Stop 이전에 수락한 FIFO 쓰기를 완료하고 스레드를 join한다. 종료 결과를 보존한 뒤 원래 종료 코드로 종료를 요청한다. 중복 종료는 같은 결과를 보존한다. Mutex가 정리 중이면 메인 이벤트 루프에서 기다리지 않는다.

명시적 DB 종료 테스트는 대기 중인 첫 쓰기와 이미 수락한 둘째 쓰기를 큐에 넣고, 블로킹 종료가 기다리는 동안 단일 스레드 async 런타임의 다른 작업이 응답함을 확인한다. 종료 후 두 응답의 서로 다른 값 17과 29를 확인한다. 실제 AI 자식 프로세스 그룹 정리는 U-20 인계다.

## 하네스와 사전 승인 변경

- 실제 어댑터 생성의 src-tauri main.rs 경계를 arch-check에 추가하고 직접 호출·가져온 별칭·생성자 함수 포인터 위반 고정 자료를 둔다. DatabasePath는 생성 능력이 없는 경로 값이라 허용한다. order-execution 직접 의존 금지 고정 자료는 기존 것을 유지한다.
- source-check에서 SystemClock 한 파일의 now만 허용한다. main.rs와 다른 플랫폼 파일의 직접 now 호출은 새 위반 고정 자료로 검사한다.
- 기존 src-tauri 조립 테스트: 승인된 build_app 서명에 Ports 준비 인자만 추가한다.
- 기존 source_check::tests::allowlists_hold_only_the_documented_paths: 정확 경로 목록에 SystemClock 한 항목 추가.
- 기존 clean-clock-env-allowlisted 고정 자료: now 호출을 UNIX_EPOCH 값으로 바꾸어 허용된 환경 읽기의 대조군을 유지. 다른 플랫폼 경로의 기대 목록에서 없어진 now 항목을 빼고 var는 유지. 실제 now 금지는 새 위반 고정 자료에서 따로 검사한다.
- 새 외부 패키지·기능·버전은 없다. 내부 의존으로 platform-macos에 application/domain, src-tauri에 application/domain/platform-macos/store-sqlite와 개발 의존 testkit-ports를 추가한다. Cargo.lock의 기존 버전 재배열은 Cargo 정규화이며 외부 패키지 집합과 버전은 유지한다.
- tauri.conf.json의 main 창은 숨김 시작. build.rs와 IPC 명령 목록은 변경하지 않는다. 기존 세 generate_context 검사 끄기를 유지하며 새 끄기는 없다.
- 추적된 새 ADR과 보호 경계 등록에 따른 Codex 설정·샌드박스 조각·Claude 훅 해시는 기존 생성기로 재생성한다.

## 인계

| 작업                              | 인계                                                                         |
| --------------------------------- | ---------------------------------------------------------------------------- |
| S-02                              | 저장된 계좌 인덱스를 saved_accounts에 제공하고 PortfolioRepository 교체      |
| S-03~S-08, S-11, S-12, S-16, S-18 | Ports 표의 저장소·키체인·감사 자리 표시 교체                                 |
| S-15                              | DB 열기 팩터리에 마이그레이션과 스키마 검증 통합                             |
| T-02, T-05~T-15                   | 읽기 포트와 갱신 작업을 실제 동기화 유스케이스로 교체                        |
| U-02                              | 준비된 MainScreenData와 DatabaseStatus의 타입 IPC 조회 및 이벤트 구독자 등록 |
| U-04, G-09                        | 실제 화면 표시와 2초 측정                                                    |
| U-14, O-07, U-24                  | 알림과 본인 확인 자리 표시 교체                                              |
| U-18, U-20, U-31                  | 트레이 상주, 실제 자식 프로세스 그룹 정리, 실제 장기 작업 응답성             |

## 검증

프로젝트 신뢰·훅 정의 신뢰·현재 세션 적용은 각각 확인 불가다. 시작 pnpm frozen install(ignore-scripts)과 lefthook install은 통과했다. 세션 적용 미확인을 중단 이유로 사용하지 않는다. Git 커밋 훅을 실행하고, 원본 지시문·tasks.md·progress.md·홈 설정·자격 증명 파일은 수정하지 않는다. 리뷰와 검사 최종 값은 docs/reviews/U-01.md에 기록한다.

## 손 변형

변형은 mktemp 작업 복사본에서 실제 실행하며 컴파일되지 않는 변형은 검출로 세지 않는다. 실행 후 아래 표에 결과를 기록한다.

원본과 복원 대조군은 각각 3개 테스트가 통과했다. 24개 시도 중 유효한 23개 모두 검출했고 생존은 0개다. 15번은 필드가 사용되지 않아 컴파일 불가이며 검출 수에서 제외한다.

| 번호 | 바꾼 것                       | 컴파일 | 잡은 테스트                                                                                                                                                                                                                                            | 결과        |
| ---- | ----------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ----------- |
| 1    | 최소 초 +1                    | 통과   | system_clock::tests::conversion_clamps_at_exact_domain_boundaries                                                                                                                                                                                      | 검출        |
| 2    | 최대 초 -1                    | 통과   | system_clock::tests::conversion_clamps_at_exact_domain_boundaries                                                                                                                                                                                      | 검출        |
| 3    | 최소 나노초 1                 | 통과   | system_clock::tests::conversion_clamps_at_exact_domain_boundaries                                                                                                                                                                                      | 검출        |
| 4    | 최대 나노초 -1                | 통과   | system_clock::tests::conversion_clamps_at_exact_domain_boundaries                                                                                                                                                                                      | 검출        |
| 5    | 양수 초 +1                    | 통과   | system_clock::tests::conversion_follows_wall_clock_backwards_without_a_monotonicity_guarantee, system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch                                                                    | 검출        |
| 6    | 양수 분수 마이크로초          | 통과   | system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch, system_clock::tests::conversion_follows_wall_clock_backwards_without_a_monotonicity_guarantee                                                                    | 검출        |
| 7    | 음수 부호 반전 제거           | 통과   | system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch                                                                                                                                                                   | 검출        |
| 8    | 음수 정수 초 -1               | 통과   | system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch                                                                                                                                                                   | 검출        |
| 9    | 음수 분수 초 -2               | 통과   | system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch                                                                                                                                                                   | 검출        |
| 10   | 음수 분수 초 그대로           | 통과   | system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch                                                                                                                                                                   | 검출        |
| 11   | 음수 보완 나노초 -1           | 통과   | system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch                                                                                                                                                                   | 검출        |
| 12   | 음수 보완 나노초 +1           | 통과   | system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch                                                                                                                                                                   | 검출        |
| 13   | 하한 비교 포함                | 통과   | system_clock::tests::conversion_clamps_at_exact_domain_boundaries                                                                                                                                                                                      | 검출        |
| 14   | 상한 비교 포함                | 통과   | system_clock::tests::conversion_clamps_at_exact_domain_boundaries                                                                                                                                                                                      | 검출        |
| 15   | 하한 고정을 상한으로          | 실패   | -                                                                                                                                                                                                                                                      | 컴파일 불가 |
| 16   | 상한 고정을 하한으로          | 통과   | system_clock::tests::conversion_clamps_at_exact_domain_boundaries                                                                                                                                                                                      | 검출        |
| 17   | UTC 오프셋 변경               | 통과   | system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch, system_clock::tests::conversion_follows_wall_clock_backwards_without_a_monotonicity_guarantee, system_clock::tests::conversion_clamps_at_exact_domain_boundaries | 검출        |
| 18   | 유효 초 +1                    | 통과   | system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch, system_clock::tests::conversion_follows_wall_clock_backwards_without_a_monotonicity_guarantee, system_clock::tests::conversion_clamps_at_exact_domain_boundaries | 검출        |
| 19   | 유효 나노초 -1                | 통과   | system_clock::tests::conversion_follows_wall_clock_backwards_without_a_monotonicity_guarantee, system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch, system_clock::tests::conversion_clamps_at_exact_domain_boundaries | 검출        |
| 20   | 유효 나노초 +1                | 통과   | system_clock::tests::conversion_clamps_at_exact_domain_boundaries, system_clock::tests::conversion_follows_wall_clock_backwards_without_a_monotonicity_guarantee, system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch | 검출        |
| 21   | 변환 실패 고정 하한           | 통과   | system_clock::tests::conversion_clamps_at_exact_domain_boundaries                                                                                                                                                                                      | 검출        |
| 22   | 음수 입력 분수를 마이크로초로 | 통과   | system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch                                                                                                                                                                   | 검출        |
| 23   | OS 변환 나노초 반전           | 통과   | system_clock::tests::conversion_follows_wall_clock_backwards_without_a_monotonicity_guarantee, system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch                                                                    | 검출        |
| 24   | 음수 정수 나노초 변경         | 통과   | system_clock::tests::conversion_preserves_fractional_instants_on_both_sides_of_epoch                                                                                                                                                                   | 검출        |

## 1차 리뷰 평가 후 결정

- 어댑터 생성 검사는 qself, 매크로, extern crate와 모듈 재수출, Cargo 의존성 별칭까지 확인한다. 경계 밖에서는 어댑터 모듈 별칭·재수출과 어댑터/한정 연관 타입의 자유 타입 별칭도 금지한다. 일반 타입 참조와 DatabasePath 값 구성은 허용한다. 별칭은 cargo metadata의 실제 의존성 이름으로 해석한다. 고정 자료는 harness/arch-check/composition-cases.toml에 있다.
- run_app은 제품의 Tauri run 루프를 그대로 사용한다. 실제 종료 효과만 main.rs에서 전달한다. 제품은 AppHandle::exit를, mock 테스트는 기록과 종료 재진입을 수행하는 함수를 전달한다. 종료 코드는 ExitCode로 보존한다. Running → Stopping → Stopped 전이는 중복 요청을 한 번의 정리로 묶으며 메인 이벤트 루프는 try_lock 실패 시 기다리지 않는다.
- 주 화면 상태가 없거나 중복 등록되면 창 표시를 거부한다. 실제 build_app과 표시 함수가 같은 불변식을 사용한다. visible=true로 시작한 mock 설정도 조립 단계에서 숨겨야 한다.
- Database::shutdown은 Writer의 Stop·join 성공 후에만 만들 수 있는 DatabaseClosed 증거를 반환한다. 이는 명시적 writer 종료와 DB 소유권 소비의 증거다. 취소된 읽기 작업의 별도 소유권까지 모두 사라졌다는 주장은 하지 않는다. 열리지 않은 DB는 NotOpened, 실제 명시적 종료는 Closed 증거로 구별한다. Drop으로 자원을 해제한 뒤 Ok(())를 반환하는 대체 구현은 성공 종료 증거가 될 수 없다.
- 갱신 작업의 panic은 Cancelled로 바꾸지 않고 Failed의 원래 런타임 오류로 보존한다.
- 제품 DB 팩터리를 main.rs의 함수 하나로 두고 제품·실제 DB 통합 테스트가 같은 함수를 사용한다. 개별 저장소 포트는 여전히 자리 표시다.
- U-02: 자리 표시의 CheckCredentials 분류를 곧바로 사용자 문구로 내보내지 않는다. 설정 부재와 실제 요청 실패를 자체 점검·설정 사실로 구별한다. 현재는 이 오류의 화면 소비 경로가 없다.
- U-20: 실제 자식 작업과 저장소 유스케이스를 연결할 때 refresh 정리 완료와 DB 종료의 상대 순서를 직접 관측하는 통합 테스트를 추가한다. 현재 코드는 취소·await 후 명시적 DB 종료 순서다.

테스트용 WebviewWindowBuilder 허용은 src/tests/exit.rs 한 파일에 한정한다. 실제 mock 런타임에서 마지막 창 종료 후 재진입 이벤트를 만들기 위한 경계다. 제품 capability와 WebView 권한은 늘리지 않는다.

## 1차 수정 손 변형

현재 수정 소스를 mktemp 복사본으로 옮겨 한 번에 하나씩 변형했다. 원본·복원 대조군 lib 29개가 각각 통과했고, 아래 8개는 전부 독립 컴파일 후 해당 테스트에서 검출됐다. 최초 시계 변형과 합치면 32개 시도 중 유효 31개 검출, 컴파일 불가 1개, 생존 0개다.

| 번호 | 바꾼 것                                  | 컴파일 | 잡은 테스트                                                                              | 결과 |
| ---- | ---------------------------------------- | ------ | ---------------------------------------------------------------------------------------- | ---- |
| 1    | 제품 종료 콜백 연결 제거                 | 통과   | tests::exit::run_app_dispatches_the_real_mock_exit_event_and_waits_for_cleanup           | 검출 |
| 2    | 종료 미루기 호출 제거                    | 통과   | tests::exit::the_product_exit_handler_defers_cleans_once_and_allows_reentry              | 검출 |
| 3    | 종료 코드 0으로 치환                     | 통과   | tests::exit::the_product_exit_handler_defers_cleans_once_and_allows_reentry              | 검출 |
| 4    | 정리 호출 제거                           | 통과   | tests::exit::the_product_exit_handler_defers_cleans_once_and_allows_reentry              | 검출 |
| 5    | 초기 창 숨김 제거                        | 통과   | tests::startup::build_app_forces_initially_visible_windows_to_remain_hidden_during_setup | 검출 |
| 6    | 주 화면 상태 등록을 표시 뒤로 이동       | 통과   | tests::startup::build_app_runs_the_same_startup_with_the_mock_runtime_and_port_fakes     | 검출 |
| 7    | DB 명시적 종료를 Drop과 NotOpened로 치환 | 통과   | tests::configured_database_is_opened_and_explicitly_closed_after_refresh_cleanup         | 검출 |
| 8    | 제품 팩터리의 폴더 준비 제거             | 통과   | tests::configured_database_is_opened_and_explicitly_closed_after_refresh_cleanup         | 검출 |

같은 파일의 서로 다른 모듈에서 같은 별칭을 쓰는 경우에는 뒤쪽 선언으로 앞쪽 경로를 지우지 않는다. 모든 가능한 별칭 경로를 보존하고 경로마다 순환을 제한해, 어느 해석에서든 실제 어댑터 구성이 가능하면 실패한다. 이는 모듈 범위를 완전히 해석하는 컴파일러 대신 보수적으로 경계를 강제하는 선택이다. module-local-alias-shadowing 반례는 수정 전 실패·수정 후 통과했다.
