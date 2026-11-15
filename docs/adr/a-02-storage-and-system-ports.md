# A-02: 저장소와 시스템 포트

- 상태: 초안

## 결정과 범위

architecture 3.3·3.7·3.8, design SEC-1·6·8·11·12, ORD-3, UX-3과 ADR 0055·0063·0065를 따른다. 구현은 application의 새 모듈과 새 테스트, domain의 새 저장 기록·위험 변경 값만 추가한다. 기존 impl과 기존 테스트를 고치지 않는다. 어댑터, A-04 가짜, 주문 전송, UI 문구는 만들지 않는다.

모든 포트는 Send + Sync와 RPITIT의 Send Future를 사용한다. Clock도 같은 호출 계약을 사용하되 단조 시계가 아닌 domain::Timestamp 벽시계 값을 반환한다. 런타임·파일·네트워크·현재 시각 직접 호출이 없다. dyn 호환성을 약속하지 않는다. UserPresence는 application::user_presence::UserPresence로 공개한다. 현재 arch-check가 정의 crate의 pub use도 UserPresenceReexport로 거부하므로 공개 모듈을 쓰고 trait을 루트에서 재내보내지 않는다. 검사나 기존 테스트를 바꾸지 않는 가장 가까운 배치다. 성공한 쓰기는 WriteCompleted, 조회 부재는 Option 또는 빈 Vec다. 쓰기 완료는 어댑터가 내구 저장을 마친 뒤 반환한다. 호출자는 완료를 받기 전에 이벤트를 내거나 주문을 전송하지 않는다.

## 계약 표

출력 열에는 Result의 성공 값만 적는다. 오류는 모든 행에서 SystemPortError다. 저장소 이름은 architecture 6.2의 묶음에 대응하며 새 저장 기능을 발명하지 않는다.

| 포트                  | 메서드            | 입력                                                      | 출력                              | 소비자·근거                                           |
| --------------------- | ----------------- | --------------------------------------------------------- | --------------------------------- | ----------------------------------------------------- |
| PortfolioRepository   | record            | AccountId, SyncRecord<Portfolio>                          | WriteCompleted                    | A-06, S-02; 성공·일부 실패·실패·접근 차단 기록        |
| PortfolioRepository   | latest            | AccountId                                                 | Option<SyncRecord<Portfolio>>     | A-08; 저장된 최신 결과 조회                           |
| TransactionRepository | record            | AccountId, SyncRecord<SettledRowSet>                      | WriteCompleted                    | A-07, S-03; 멀티셋과 사라진 행 표시 보존              |
| TransactionRepository | latest            | AccountId                                                 | Option<SyncRecord<SettledRowSet>> | A-07·A-16; 기존 대조 결과 조회                        |
| MarketDataRepository  | store_quote       | InstrumentId, Quote                                       | WriteCompleted                    | S-04; 현재가 사실 보존                                |
| MarketDataRepository  | quote             | InstrumentId                                              | Option<Quote>                     | A-09·A-14; 최신 관측 조회                             |
| MarketDataRepository  | store_candles     | InstrumentId, [Candle]                                    | WriteCompleted                    | S-04; 원시·수정·일봉·분봉 사실 보존                   |
| MarketDataRepository  | candles           | InstrumentId, TimeInterval, CandlePeriod, PriceAdjustment | Vec<Candle>                       | A-15·A-16; 기간·수정 기준이 같은 자료만 반환          |
| MarketDataRepository  | store_fx_rate     | FxRate                                                    | WriteCompleted                    | S-04; 종류·출처·시각 보존                             |
| MarketDataRepository  | fx_rate           | FxPair, FxRateKind, Option<Timestamp>                     | Option<FxRate>                    | A-08·A-16; None은 최신, Some은 그 시각 이하 최신 사실 |
| NewsRepository        | store             | NewsItem                                                  | WriteCompleted                    | S-05; 봉인된 기사 보존                                |
| NewsRepository        | for_instrument    | InstrumentId                                              | Vec<NewsItem>                     | A-13; 종목별 근거 조회                                |
| ThesisRepository      | store             | InstrumentId, Thesis                                      | WriteCompleted                    | S-06; 이력을 포함한 노트 보존                         |
| ThesisRepository      | load              | InstrumentId                                              | Option<Thesis>                    | A-13; 현재 노트와 이력 조회                           |
| EstimateRepository    | append_submission | SignalSubmission                                          | WriteCompleted                    | A-12; 거래 여부와 무관하게 추가                       |
| EstimateRepository    | submissions       | InstrumentId                                              | Vec<SignalSubmission>             | A-13·A-15; 종목별 추정 목록                           |
| EstimateRepository    | append_resolution | SignalTaskKey, EstimateOutcome                            | WriteCompleted                    | A-15; 같은 과제의 두 번째 판정은 Conflict             |
| EstimateRepository    | resolution        | SignalTaskKey                                             | Option<EstimateOutcome>           | A-15; 판정한 과제는 다시 바꾸지 않음                  |
| EstimateRepository    | store_calibration | CalibrationKey, Calibration                               | WriteCompleted                    | A-15; 모델·기간별 보정 자료                           |
| EstimateRepository    | calibration       | CalibrationKey                                            | Option<Calibration>               | A-13; 기존 자료 조회                                  |
| SignalRepository      | replace           | InstrumentId, [RaisedSignal]                              | WriteCompleted                    | A-13, S-07; 빈 목록은 현재 신호 해제                  |
| SignalRepository      | current           | InstrumentId                                              | Vec<RaisedSignal>                 | A-14; 종목별 현재 신호                                |
| ProposalRepository    | store_draft       | Proposal<Draft>, ValidationReport                         | WriteCompleted                    | A-14, S-07; 검증 권한은 저장하지 않음                 |
| ProposalRepository    | draft             | ProposalId                                                | Option<Proposal<Draft>>           | O-04; 승인 시 반드시 재검증                           |
| ValuationRepository   | append            | AccountId, ValuationSnapshot                              | WriteCompleted                    | A-16, S-18; 평가 기록 추가 전용                       |
| ValuationRepository   | history           | AccountId, TimeInterval                                   | Vec<ValuationSnapshot>            | A-16; 빠진 날 재계산과 성과 조회                      |
| SettingsRepository    | store_profile     | InvestorProfile                                           | WriteCompleted                    | A-17, S-10; 확인을 마친 성향 저장                     |
| SettingsRepository    | profile           | 없음                                                      | Option<InvestorProfile>           | A-08·A-13; 부재는 호출자가 기본값 사용                |
| AuditLog              | append            | AuditEntry                                                | WriteCompleted                    | O-05, S-08; 내구 저장 실패 시 전송 금지               |
| AuditLog              | verify            | 없음                                                      | AuditIntegrity                    | S-16; Verified 또는 Tampered, 검증 자체 실패는 Err    |
| SecretStore           | load              | SecretKey                                                 | Option<SecretValue>               | T-02, S-12; 미등록과 접근 실패 구분                   |
| SecretStore           | store             | SecretKey, SecretValue 소유권                             | WriteCompleted                    | T-02, S-12; 비밀을 복제하지 않음                      |
| SecretStore           | remove            | SecretKey                                                 | WriteCompleted                    | T-02, S-12; 부재 삭제는 성공                          |
| Clock                 | now               | 없음                                                      | Timestamp                         | J-01, A-06~A-16; 벽시계만 제공                        |
| Notifier              | notify            | Notification                                              | WriteCompleted                    | U-14; 새 신호·긴급 공시·브리핑 완료                   |
| UserPresence          | confirm           | ConfirmationRequest                                       | ConfirmationOutcome               | A-17, O-17; 주문 요약 또는 위험 변경                  |

저장소의 계좌·종목 키와 자료의 결합, 정렬, 트랜잭션, 중복 거부와 전체 이력 보존은 해당 S 어댑터 계약에서 검증한다. 이 trait 선언 자체가 DB의 원자성이나 외부 사실의 진위를 인증하지 않는다. 조회 목록은 시각 오름차순이며 동일 시각 기록의 안정적 순서는 어댑터가 보존한다. 판정·평가 추가에서 기존 행을 덮어쓰지 않는다. store_calibration도 같은 키의 새 보정표 버전을 추가하며 calibration은 최신 버전을 반환한다. 과거 보정표를 덮어쓰거나 지우지 않는다. SyncRecord는 시각과 네 결과를 보존하며 실패를 0원이나 빈 포트폴리오로 바꾸지 않는다. 마지막 성공 자료의 조회 정책과 실패 원인 상세는 A-06·A-07에서 확정한다.

## 비밀과 의존성

사용자는 이번 회차에서 secrecy 및 zeroize 추가와 필요한 보호 경로의 등록 변경을 승인했다. secrecy는 =0.10.3, 기본 기능만 사용하고 serde는 켜지 않는다. 전이 zeroize는 Cargo.lock의 정확한 버전과 체크섬으로 고정하며 alloc 기능만 사용한다. 둘 다 Apache-2.0 OR MIT이며 deny의 기존 라이선스 정책 안에 있다. 의존성 허용 목록에 두 crate만 추가한다. 다른 새 외부 crate는 추가하지 않는다.

SecretValue의 비공개 필드는 secrecy::SecretSlice<u8>다. 토큰 텍스트뿐 아니라 DB·HMAC 원시 키도 손실 없이 보관한다. 생성자는 이미 비밀로 감싼 바이트의 소유권을 받는다. 유일한 노출 메서드는 as_exposed_secret이며 빌린 바이트만 반환한다. Deref, AsRef, 변환·복제·직렬화를 제공하지 않는다. Debug는 직접 쓴 구현으로 SecretValue(<redacted>) 한 문자열만 출력하며 self나 비밀 길이를 읽지 않는다. derive(Debug)는 쓰지 않는다. secrecy가 Drop에서 zeroize를 수행한다. 반환된 참조를 어댑터가 복사하거나 HTTP 라이브러리에 전달한 뒤의 수명·헤더 set_sensitive는 T-02·S-12 책임이다.

메인은 SEC-11의 비밀 유출 방지 취지를 고정 Debug 출력으로 지키도록 결정했다. 하네스의 예외 불가 missing_debug_implementations deny는 그대로 유지하고 expect를 제거한다. Debug 출력은 일반·pretty 형식과 값·길이에 무관한 상수이므로 비밀을 읽거나 출력하는 경로가 아니다. docs/design.md SEC-11과 docs/architecture.md 3.3을 이에 맞게 갱신하며 tasks.md A-02 문구는 메인이 병합 때 고친다.

원시 타입 검사에는 SecretSlice<u8> 전용 newtype 필드·빌린 &[u8] 대응만 등록한다. 일반 원시 타입 허용 범위는 넓히지 않는다. Display·Clone·Serialize 부재는 외부 컴파일 실패와 각 trait 양성 대조군으로 고정한다. Debug 부재 실험은 제거한다. 새 dev 의존성을 앱에 추가하지 않으며 외부 시험 crate에서 저장소가 이미 사용하는 고정 serde와 proptest 버전으로 직렬화 경계와 속성 테스트를 확인한다. 시험은 오프라인으로 실행하고 자체 임시 폴더만 지운다.

## 확인·알림·오류의 경계

주문 확인은 기존 봉인된 OrderSummary를 빌린다. 종목 이름·코드·시장·방향·수량·가격·예상 금액·수수료·계좌 뒤 4자리·주문 키 앞부분을 그대로 보존한다. 요약 창과 시스템 이유 문자열 구성은 O-07 책임이다. 위험 변경은 EnableOrders·AllowMarketOrders·AllowHours·RelaxWeightCaps·IncreaseRiskAppetite 다섯 종류와 목표 값만 받는다. 변경 방향·기존 값 비교와 실제 저장은 A-17이 담당한다. Confirmed는 저장·복원할 승인 토큰이 아니며 macOS 호출이나 주문 전송 권한도 아니다. Denied와 Unavailable을 서로 바꾸지 않는다.

Notification은 페이로드 없는 세 종류다. 임의 본문·종목명·금액·계좌를 입력할 수 없으며 U-14가 종류별 고정 한국어 문구를 사용한다. 항목별 상세 안내는 앱 안의 재조회로 처리한다.

SystemPortError는 포트 종류와 닫힌 원인만 가진다. PermissionDenied는 공통 권한 확인 조치, Unavailable은 원래 RetryDelay, InvalidData·Conflict·IntegrityViolation은 검증 실패 버그다. 토스 HTTP 오류로 변환하지 않는다. 임의 문자열·외부 오류·비밀·본문이 없다. 조회 부재는 오류 대신 정상 Option·Vec로 표현한다. 새 사용자 조치 종류가 필요한 경우 A-05 후속으로 추가하며 기존 exhaustive 테스트를 약하게 바꾸지 않는다.

## 정의 대기와 인계

| 저장소·계약           | 아직 없는 타입·메서드                                                                 | 받는 작업                      |
| --------------------- | ------------------------------------------------------------------------------------- | ------------------------------ |
| 거래내역 원본 저장    | 원본 응답·격리 행의 봉인된 영속 자료, 부분 대조의 원인·창 기록                        | A-07, S-03                     |
| 원가·현금             | 로트·현금 원장·기업 행위·섹터 대응표의 최소 조회·재구축 계약                          | S-03, A-16, O-11               |
| 뉴스·공시             | 공시 목록·검색 질의·묶음·전체 소스 키와 저장·검색 메서드                              | S-05, N-01, N-09               |
| 수집 결과             | 실행·항목 ID와 실행별 수집 결과 보존·조회                                             | AI-14, S-09                    |
| 추정 과제·실행 배정   | 봉인된 과제 ID·실행 ID, 배정·미제출·두 번째 제출 거부의 원자 계약                     | A-12, AI-17, S-09              |
| 신호·제안             | 소비 기록과 전송 전 감사 기록을 하나의 트랜잭션으로 쓰는 승인 포트                    | O-18, S-07, S-08               |
| 주문                  | SubmittedOrder·결과·외부 주문 관측의 영속 복원 계약                                   | O-11, O-19                     |
| AI 작업·MCP 원장·대화 | 실행 ID, 사용량·메타데이터·메시지·인자/결과 해시의 봉인 타입과 추가 전용 계약         | AI-06, AI-08, S-09, M-02, C-02 |
| 설정                  | 주문 스위치·시장가 허용의 봉인 상태, 주기·모델·알림·토스 스펙 해시                    | A-17, S-10, O-06               |
| 감사 로그 확장        | 추정·검증 상세·전송 응답·위험 변경 확인 결과·검색어 검사 원장·조회와 체인 머리값 타입 | O-05, S-08, S-16, N-19         |
| 시스템 본인 확인      | 시스템 이유 문자열 한도·서명 앱 동작 실측                                             | O-07, S-17, 사용자             |

## 미해결·사용자 확인 대상

추정 과제는 현재 SignalTaskKey라는 봉인된 사실 묶음으로 식별하며 실행 배정 권한을 뜻하지 않는다. A-12의 과제·실행 식별자가 정해지면 저장소 계약을 확정한다. 시간·수정 기준을 포함한 캐시 조회, 계좌·종목 결합과 중복·부분 실패 정책은 S 어댑터 테스트에서 입증해야 한다. 포트만 정의한 이번 회차를 저장소·키체인·본인 확인의 실행 검증으로 보지 않는다. 실제 키·계좌·API와 macOS 본인 확인 실측은 수행하지 않는다.

## 검증과 인계

실제 작업 사본에서 새 테스트를 먼저 넣었을 때 E0432·E0433으로 종료코드 101을 확인한 뒤 포트를 구현했다. 고정 Debug 결정 뒤에는 일반·pretty·부모 Debug 테스트가 E0277로 컴파일되지 않음을 먼저 확인한 뒤 수동 구현을 넣었다. cargo test --locked -p application의 테스트는 기존·신규 테스트와 2,306개 바이트/길이 조합의 고정 출력 검사를 포함한다.

외부 시험은 Display·Clone·Serialize 각 trait 양성 대조군과 SecretValue의 E0277 거부, 비공개 필드의 E0616 거부, 기본 기능과 serde 미활성화를 확인한다. Debug 속성 테스트 두 개는 각각 256회 임의 입력으로 값·길이에 무관한 일반·pretty 고정 출력과 derive(Debug) 부모의 고정 출력, 표식 붙인 비밀 원문·바이트 배열 표현의 부재를 확인한다. 모든 바이트 입력에 대해 출력이 동일한 상수라는 비간섭 조건을 사용하며 상수의 문자와 비밀의 한 바이트가 우연히 같은 것을 누출로 취급하지 않는다. 실행 명령은 node --test crates/application/tests/secret_traits.mjs다. 이 별도 회귀는 Cargo의 Rust 테스트 발견 대상이 아니므로 just check 결과와 구분한다.

정확한 두 패키지 추가에 대해 just deny의 advisories·bans·licenses·sources가 통과했다. secrecy 0.10.3, 전이 zeroize 1.9.1만 잠금 파일에 추가됐으며 zeroize의 alloc 이외 기능과 secrecy의 serde를 켜지 않았다.

변이 선택 코드를 확인하면 application/Cargo.toml과 통합 테스트 변경은 non-target crate input이므로 crate 전체로 넓어진다. 자동 변이는 실행하지 않았다. mktemp로 만든 전용 복사본에서 원본 대조군을 통과시킨 뒤, 포트·오류 원인의 생성/조회 매핑, 오류 분류와 대기 시간 유실, 비밀 바이트의 빈 값 치환과 첫 바이트 누락 등 25개 변형을 각각 컴파일하고 실행했다. 최종 결과는 검출 25, 생존 0, 컴파일 불가 0, 시간 초과 0이다. const fn 제약 때문에 컴파일되지 않은 첫 시도는 이 수에 포함하지 않았다. 임시 파일의 복사 시각 때문에 남은 이전 바이너리가 재사용되는 것을 막고 원본 대조군부터 다시 통과시켰다.

A-04·A-17·O-17은 공개 모듈 경로 application::user_presence::UserPresence를 사용한다. 현재 AuditEntry는 닫힌 종류·식별자·확인 결과·설정 목표를 보존하는 최소 계약이다. 이를 ORD-3의 전체 상세 증거로 취급하지 않으며, O-05·S-08·S-16이 정의 대기 표의 불변 상세 기록과 원자적 승인 기록을 확정한 뒤 전송 경로를 구현한다.

프로젝트 설정과 세션 훅 신뢰 적용 자체는 미확인이다. 실제 Git 훅과 직접 실행한 검사 결과를 검증 근거로 사용한다. 사용자 지시대로 하위 에이전트나 독립 리뷰를 실행하지 않았다. 독립 적대적 리뷰와 비판은 메인의 후속 절차다.

## 메인 결정 반영

앞 회차 전체 검사는 개별 deny를 끄는 expect가 disable-check에 거부되어 중단됐다. 메인은 하네스 예외 대신 비밀 내용과 길이에 독립적인 수동 Debug를 승인했다. Debug가 없다는 이전 완료 기준과 실험은 이 결정으로 대체한다. Debug 예외를 위해 하네스의 허용 목록·검사 코드·기존 테스트를 바꾸지 않으며 새 검사 끄기도 없다. 정상 훅으로 전체 검사와 커밋을 재개한다.

의존성 등록은 기존 cargo-deny의 bans.allow와 잠금 파일에 한정한다. 초안에 추가했던 application 전용 external-allowlist는 반례 케이스가 없다는 기존 하네스 회귀에 검출됐으며, 요구사항에 없는 추가 구조 정책이므로 최종 변경에서 제거했다. 기존 구조 규칙과 검사·테스트는 그대로 유지한다.

외부 속성 테스트의 Rust 소스는 Node 시험 생성기 안에 보관하고 임시 crate의 test target으로 생성한다. 작업 공간 Cargo가 컴파일하지 않는 .rs 파일을 crates/application/tests 아래에 두면 test-diff가 unreachable로 거부하므로 이 배치를 사용한다. 속성 테스트 두 개는 생성 후 cargo test로 실제 실행하며, 작업 공간 자체에는 도달 가능한 secret_debug 통합 테스트와 2,306개 고정 조합 검사가 있다.
