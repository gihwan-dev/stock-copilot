# A-02: 저장소와 시스템 포트

- 상태: 초안

## 결정과 범위

architecture 3.3·3.7·3.8, design SEC-1·6·8·11·12, ORD-3, UX-3과 ADR 0055·0063·0065를 따른다. 이번 리뷰 수정은 메인이 확정한 단순화 결정 1~10을 반영한다. 어댑터, A-04 가짜, 주문 전송과 UI는 구현하지 않는다. 실제 키·계좌·API는 사용하지 않는다.

Clock은 동기이며 실패하지 않는다. 나머지 포트는 Send + Sync와 RPITIT의 Send Future를 사용한다. UserPresence의 두 메서드는 Result 없이 각자의 닫힌 결과를 반환한다. 저장·조회 포트의 오류는 SystemPortError이며 ClassifiedError를 구현한다. 조회 부재는 Option 또는 빈 Vec로 표현한다. 임의 문자열·HTTP 응답·외부 오류 객체를 오류에 넣지 않는다.

## 최종 계약 표

성공 값 열은 Result의 성공 값만 나타낸다. Clock과 UserPresence는 표의 값을 직접 반환한다. UserPresence는 application::user_presence::UserPresence로 공개하며 루트 재내보내기는 하지 않는다.

| 포트 | 메서드 | 입력 | 성공 값 | 소비자·근거 |
|---|---|---|---|---|
| PortfolioRepository | record | AccountId, SyncRecord<Portfolio> | WriteCompleted | A-06, S-02; 성공·일부 실패·실패·접근 차단 기록 |
| PortfolioRepository | latest | AccountId | Option<SyncRecord<Portfolio>> | A-08; 저장된 최신 결과 조회 |
| TransactionRepository | record | AccountId, SyncRecord<SettledRowSet> | WriteCompleted | A-07, S-03; 멀티셋과 사라진 행 표시 보존 |
| TransactionRepository | latest | AccountId | Option<SyncRecord<SettledRowSet>> | A-07·A-16; 기존 대조 결과 조회 |
| MarketDataRepository | store_quote | InstrumentId, Quote | WriteCompleted | S-04; 현재가 사실 보존 |
| MarketDataRepository | quote | InstrumentId | Option<Quote> | A-09·A-14; 최신 관측 조회 |
| MarketDataRepository | store_candles | InstrumentId, [Candle] | WriteCompleted | S-04; 원시·수정·일봉·분봉 사실 보존 |
| MarketDataRepository | candles | InstrumentId, TimeInterval, CandlePeriod, PriceAdjustment | Vec<Candle> | A-15·A-16; 기간·수정 기준이 같은 자료만 반환 |
| MarketDataRepository | store_fx_rate | FxRate | WriteCompleted | S-04; 종류·출처·시각 보존 |
| MarketDataRepository | fx_rate | FxPair, FxRateKind, FxRateQuery | Option<FxRate> | A-08·A-16; Latest는 최신, At은 그 시각 이하 최신 사실 |
| NewsRepository | store | NewsItem | WriteCompleted | S-05; 봉인된 기사 보존 |
| NewsRepository | for_instrument | InstrumentId | Vec<NewsItem> | A-13; 종목별 근거 조회 |
| ThesisRepository | store | InstrumentId, Thesis | WriteCompleted | S-06; 이력을 포함한 노트 보존 |
| ThesisRepository | load | InstrumentId | Option<Thesis> | A-13; 현재 노트와 이력 조회 |
| EstimateRepository | append_submission | SignalSubmission | RecordOutcome | A-12; 거래 여부와 무관하게 추가 |
| EstimateRepository | submissions | InstrumentId | Vec<SignalSubmission> | A-13·A-15; 종목별 추정 목록 |
| EstimateRepository | append_resolution | SignalTaskKey, EstimateOutcome | RecordOutcome | A-15; 같은 과제의 두 번째 판정은 AlreadyRecorded |
| EstimateRepository | resolution | SignalTaskKey | Option<EstimateOutcome> | A-15; 판정한 과제는 다시 바꾸지 않음 |
| EstimateRepository | append_calibration | CalibrationKey, Calibration | RecordOutcome | A-15; 모델·기간별 보정 자료 |
| EstimateRepository | calibration | CalibrationKey | Option<Calibration> | A-13; 기존 자료 조회 |
| SignalRepository | replace | InstrumentId, [RaisedSignal] | WriteCompleted | A-13, S-07; 빈 목록은 현재 신호 해제 |
| SignalRepository | current | InstrumentId | Vec<RaisedSignal> | A-14; 종목별 현재 신호 |
| ProposalRepository | store_draft | Proposal<Draft> | WriteCompleted | A-14, S-07; 검증 권한은 저장하지 않음 |
| ProposalRepository | draft | ProposalId | Option<Proposal<Draft>> | O-04; 승인 시 반드시 재검증 |
| ValuationRepository | append | AccountId, ValuationSnapshot | RecordOutcome | A-16, S-18; 평가 기록 추가 전용 |
| ValuationRepository | history | AccountId, TimeInterval | Vec<ValuationSnapshot> | A-16; 빠진 날 재계산과 성과 조회 |
| SettingsRepository | store_profile | InvestorProfile | WriteCompleted | A-17, S-10; 확인을 마친 성향 저장 |
| SettingsRepository | profile | 없음 | Option<InvestorProfile> | A-08·A-13; 부재는 호출자가 기본값 사용 |
| SettingsRepository | store_risk_settings | &RiskSettings | WriteCompleted | A-17, S-10; 확인한 위험 설정을 저장 |
| SettingsRepository | risk_settings | 없음 | Option<RiskSettings> | A-17, S-10; 변경 전 전체 위험 설정 조회 |
| AuditLog | append | AuditEntry | RecordOutcome | O-05, S-08; 내구 저장 실패 시 전송 금지 |
| AuditLog | verify | 없음 | AuditIntegrity | S-16; Verified 또는 Tampered, 검증 자체 실패는 Err |
| SecretStore | load | SecretKey | Option<SecretValue> | T-02, S-12; 미등록과 접근 실패 구분 |
| SecretStore | store | SecretKey, SecretValue 소유권 | WriteCompleted | T-02, S-12; 비밀을 복제하지 않음 |
| SecretStore | remove | SecretKey | WriteCompleted | T-02, S-12; 부재 삭제는 성공 |
| Clock | now | 없음 | Timestamp | J-01, A-06~A-16; 벽시계만 제공 |
| Notifier | notify | Notification | WriteCompleted | U-14; 새 신호·긴급 공시·브리핑 완료 |
| UserPresence | confirm_order | OrderSummary | OrderConfirmation | O-17; 주문만 본인 확인 |
| UserPresence | confirm_risk_change | RiskChange | RiskChangeConfirmation | A-17; 위험 변경 확인 |

## 중복 기록과 무결성

append 계열은 RecordOutcome::Recorded 또는 AlreadyRecorded를 정상 결과로 돌려준다. EstimateRepository의 같은 SignalTaskKey에 대한 두 번째 판정은 기존 판정을 바꾸지 않고 AlreadyRecorded다. append_calibration은 같은 키의 새 보정표 버전을 추가하고 calibration은 최신 버전을 반환한다. 과거 버전을 덮어쓰지 않는다. 동일한 기록의 재요청은 AlreadyRecorded이며 별도 논리 기록의 정렬·식별·원자성은 S 어댑터가 보장한다. AuditLog의 같은 AuditEntry 재요청도 추가 중복을 만들지 않는다.

SystemFailure는 PermissionDenied, Unavailable(RetryDelay), InvalidData, RepositoryIntegrity, SecretIntegrity다. PermissionDenied는 권한 확인 조치, Unavailable은 원래 대기 시간이다. 나머지는 각각 BugDiagnostic::System의 InvalidStoredData, RepositoryIntegrity, SecretIntegrity에 대응한다. 토스 ResponseMismatch를 빌리지 않는다. SystemPort는 실제 실패 가능한 Repository·AuditLog·SecretStore·Notifier만 구분한다.

RepositoryIntegrity는 저장소 자체의 손상·일관성 실패이며 감사 로그 체인 내용의 변조 판정이 아니다. 감사 로그의 체인 변조는 Ok(AuditIntegrity::Tampered)로만 반환한다. generic IntegrityViolation이나 중복용 Conflict 오류는 없다. verify에서 저장 장치의 읽기 자체가 실패한 경우는 정상 변조 판정을 만들지 않고 I/O 실패를 반환한다. 감사 기록의 불변 상세 자료·원자적 승인 기록은 O-05·S-08·S-16이 확정한다.

## 본인 확인과 위험 설정

confirm_order는 봉인된 OrderSummary를 받고 OrderConfirmation을 반환한다. confirm_risk_change는 RiskChange를 받고 RiskChangeConfirmation을 반환한다. 두 결과는 서로 다른 타입이며 각각 Confirmed·Denied·Unavailable만 가진다. 본인 확인 불가는 Unavailable 한 경로로만 반환하며 별도 오류는 없다. 위험 설정 확인 결과를 주문 결과로 전달할 수 없다. Confirmed 값은 주문 전송 권한이나 저장 가능한 승인 토큰이 아니다.

RiskChange는 EnableOrders·AllowMarketOrders·AllowHours·RelaxWeightCaps(WeightCaps)·IncreaseRiskAppetite(RiskAppetite)의 닫힌 요청이다. RiskSettings는 OrderPermission·SavedMarketOrderSetting·HoursPermissionState·WeightCaps·RiskAppetite 다섯 값을 저장한다. D-12의 WeightCaps·RiskAppetite, 기존 HoursPermissionState와 이번에 master로 받은 D-26의 SavedMarketOrderSetting을 재사용하고, 없던 주문 스위치만 최소 닫힌 타입 OrderPermission으로 추가했다. 공개 필드나 불린은 없다.

A-17은 저장된 위험 설정과 새 값을 비교하고 위험을 늘리는 변경을 확인한 뒤 저장한다. store_risk_settings는 다섯 설정을 한 번에 저장한다. profile이 노출하는 비중·위험 성향·몇 시간 단위 기간 설정은 같은 저장 사실을 반영해야 하며 별도 값으로 어긋나게 저장하지 않는다. 같은 사실의 조립과 트랜잭션은 S-10의 책임이다. 초기 부재를 처리하는 A-17은 주문·시장가의 안전한 초기값과 기존 기본 투자 성향을 사용한다.

AuditKind::RiskChangeConfirmationRecorded는 RiskChange와 RiskChangeConfirmation을 함께 보존한다. Denied·Unavailable도 기록하며 실제 변경 기록 RiskSettingsChanged와 구분한다. 주문용 PresenceRecorded는 OrderConfirmation만 받는다. A-04의 ScriptedPresence는 주문과 위험 변경을 별도 메서드·별도 결과 큐로 다루고 요청 종류별 호출을 기록해야 한다.

## 비밀 값과 위치 등록

SecretValue는 비공개 SecretSlice<u8> 필드 하나를 가진 application 타입이다. new(Vec<u8>)는 소유한 벡터를 이동해 내부에서 secrecy로 감싼다. 공개 생성자와 접근자에 secrecy 타입을 노출하지 않는다. 유일한 노출 메서드는 as_exposed_secret이며 빌린 &[u8]를 반환한다. Debug는 직접 쓴 구현으로 값·길이에 무관한 SecretValue(<redacted>)만 출력한다. 일반·pretty 형식과 이를 담은 derive(Debug) 타입에서도 동일하다. missing_debug_implementations deny를 유지하며 검사 끄기는 추가하지 않는다.

SecretKey와 SecretStore는 secret_port.rs로 분리한다. secret_store.rs에는 derive가 없고 SecretValue의 고유 impl과 수동 Debug impl 두 개만 둔다. 키는 TossClientId·TossClientSecret·TossAccessToken·DatabaseEncryptionKey·AuditHmacKey·DartApiKey·FredApiKey다. client_id도 키체인에 저장한다.

secrecy =0.10.3은 application에만 직접 의존할 수 있다. serde 기능은 켜지 않는다. 전이 zeroize 1.9.1은 alloc만 사용한다. 두 패키지의 라이선스는 Apache-2.0 OR MIT다. 다른 제품 crate는 application의 SecretValue를 사용한다. arch-check에 허용 사례 application과 거부 사례 mcp-server를 추가해 위치 규칙 자체의 회귀를 고정한다.

새 외부 패키지는 추가하지 않는다. 직렬화 trait 부재의 컴파일 단언에는 작업 공간에 이미 잠긴 serde_core =1.0.229를 기능 없는 개발 의존성으로 사용한다. 작업 공간 serde 기능 통합 때문에 메타데이터에 보이는 syn·proc-macro2 경로는 기존 proptest 등록과 같은 방식으로 serde_core 경유 경로만 등록한다. 직접 proc macro 의존은 계속 금지한다. 원시 타입 형식 표에는 SecretSlice<u8>의 생성자 입력 Vec<u8> 대응만 추가한다. 이는 비밀 공개 생성자의 소유권 이동 경계이며 다른 원시 타입 허용 범위를 넓히지 않는다.

## 조회와 저장의 단순화

fx_rate는 Option<Timestamp> 대신 FxRateQuery::Latest 또는 At(Timestamp)를 받는다. At은 지정 시각 이하 최신 사실이며 시각을 보존한다. candles는 TimeInterval·CandlePeriod·PriceAdjustment를 함께 받는다. 계좌·종목 결합과 정렬은 저장소 어댑터가 검증한다.

ProposalRepository는 Proposal<Draft>만 저장·조회한다. A-14가 만드는 제안의 사실은 저장하되 ValidationReport는 저장하지 않는다. O-04에서 최신 사실로 반드시 다시 검증하므로 과거 검증 권한이나 보고서를 복원하지 않는다. Notification은 페이로드 없는 NewSignal·UrgentDisclosure·BriefingFinished뿐이다.

## Rust 비밀 보호 검사

Cargo가 발견하는 secret_protection 통합 테스트는 참조 수신 타입과 마커 추론의 모호성을 이용해 trait 구현이 생기면 컴파일이 실패하도록 한다. Clone·Copy·Display·Serialize·Deserialize·DeserializeOwned·AsRef<[u8]>·AsMut<[u8]>·Deref·DerefMut·PartialEq·Eq·Hash·PartialOrd·Ord·Default·From<Vec<u8>>·Into<Vec<u8>>·Borrow<[u8]>를 각각 검사한다. 소스 계약은 derive 부재, 두 impl만 존재, 비공개 필드, 공개 메서드 두 개와 생성자·노출 메서드의 정확한 서명을 고정한다.

secret_debug는 2,306개 바이트·길이 조합을 검사한다. secret_properties의 두 proptest는 testkit::property_cases()를 사용해 로컬에서 각각 256회, 전체 검사에서 각각 4096회 임의 입력으로 일반·pretty·상위 Debug의 고정 출력과 표식·바이트 배열 표현의 부재를 확인한다. 모든 입력에서 출력이 동일한 상수라는 비간섭 조건이며 상수의 한 문자와 비밀의 한 바이트가 우연히 같은 것은 누출로 취급하지 않는다. 이전 secret_traits.mjs는 삭제한다. 모든 비밀 보호 검사가 cargo test -p application과 just check의 테스트 컴파일·실행에 포함된다.

## 정의 대기·인계

| 계약 | 인계 |
|---|---|
| 위험 변경 비교·확인 후 저장·부재 초기값 | A-17 |
| 요청 종류별 호출 기록과 별도 결과 큐 | A-04 ScriptedPresence |
| DB 내구성·정렬·원자성·위험 설정과 투자 성향의 일관된 조회 | S-02~S-10 |
| 감사 상세 기록·추정 실행 식별자·원자적 승인 기록·체인 머리값 | O-05, O-18, S-08, S-09, S-16 |
| 원가·현금 원장·기업 행위와 거래 원본·격리 행 저장 | A-07, A-16, O-11, S-03 |
| 뉴스·공시 전체 질의·실행별 수집 결과·MCP·대화 영속 기록 | S-05, S-09, AI-06, AI-08, AI-14, M-02, C-02 |
| 주기·모델·알림 설정과 토스 스펙 해시 | A-17, S-10, O-06 |
| 실제 키체인·macOS 본인 확인 실측 | S-12, S-17, O-07, 사용자 |

## 승인된 기존 테스트 변경

| 파일·테스트 | 전 → 후 |
|---|---|
| application/tests/error_class.rs: check_diagnostic_fields, classification_fields_are_closed_enums_or_time_span | BugDiagnostic 세 변형 → System의 명시적 분기와 세 닫힌 진단 기대 항목 추가. 기존 단언 유지 |
| application/tests/ports_a02.rs: 오류·키·확인·감사 소진 검사 | 새 닫힌 타입·서명에 맞는 분기·정확한 키 목록·확인 결과·감사 변형으로 갱신. 필드 보존 단언 유지 |
| application/tests/ports_a02_contract.rs: 네 계약 검사 | 동기 Clock, 확인 메서드 둘, 추가 전용 정상 결과, 위험 설정 두 메서드, 쿼리 타입, 보고서 없는 제안의 정확한 서명으로 갱신 |
| application/tests/secret_debug.rs: 고정 출력 검사 | 생성자 입력 SecretSlice 변환 → 소유한 Vec 직접 전달. 출력·바이트 보존 단언 유지 |
| application/tests/secret_traits.mjs | 비자동 Node 시험 → Cargo가 발견하는 Rust 타입 단언·소스 계약·속성 테스트로 이전 |

메인은 승인된 닫힌 변형 추가에 필요한 소진 분기·기대 목록 추가, 승인된 이름·서명에 따른 호출부 수정과 정확한 목록·개수 갱신을 허용했다. 와일드카드 추가·단언 삭제·범위 완화는 하지 않는다. 최종 check-fast가 이 승인된 기계적 변경만 test-diff로 지적하면 그 외 선택된 검사를 통과시킨 뒤 해당 커밋 한 번에만 LEFTHOOK=0을 적용하고 커밋 메시지에 사유를 남기도록 승인했다. 하네스의 test-diff 규칙과 기존 단언 자체는 유지한다.

## 손 변형 검증

| 번호 | 손 변형 | 라이브러리 대조 컴파일 | Rust 보호·회귀 검사 | 결과 |
|---|---|---|---|---|
| 1 | construct-port-Repository | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 2 | read-port-Repository | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 3 | construct-port-AuditLog | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 4 | read-port-AuditLog | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 5 | construct-port-SecretStore | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 6 | read-port-SecretStore | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 7 | construct-port-Notifier | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 8 | read-port-Notifier | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 9 | construct-cause-PermissionDenied | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 10 | read-cause-PermissionDenied | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 11 | construct-cause-InvalidData | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 12 | read-cause-InvalidData | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 13 | construct-cause-RepositoryIntegrity | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 14 | read-cause-RepositoryIntegrity | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 15 | construct-cause-SecretIntegrity | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 16 | read-cause-SecretIntegrity | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 17 | permission-wait | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 18 | lost-known-delay | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 19 | wrong-diagnostic-InvalidStoredData | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 20 | wrong-diagnostic-RepositoryIntegrity | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 21 | wrong-diagnostic-SecretIntegrity | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 22 | required-clone | 0 | 컴파일 모호성 E0283, 종료 101 | 검출 |
| 23 | required-public-field | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 24 | required-display | 0 | 컴파일 모호성 E0283, 종료 101 | 검출 |
| 25 | required-asref | 0 | 컴파일 모호성 E0283, 종료 101 | 검출 |
| 26 | required-partialeq | 0 | 컴파일 모호성 E0283, 종료 101 | 검출 |
| 27 | debug-constant | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 28 | debug-bytes | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 29 | debug-length | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 30 | debug-pretty | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 31 | secret-empty | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 32 | secret-drop-first | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 33 | risk-orders | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 34 | risk-market_orders | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 35 | risk-hours | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 36 | risk-risk_appetite | 0 | 테스트 단언 실패, 종료 101 | 검출 |
| 37 | risk-caps | 0 | 테스트 단언 실패, 종료 101 | 검출 |

원본 대조군과 모든 변형을 복구한 대조군은 통과했다. 37개 모두 라이브러리가 독립적으로 컴파일되는 유효 변형이며 검출 37·생존 0·컴파일 불가 0·시간 초과 0이다. Clone·Display·AsRef·PartialEq는 보호 단언이 추가 trait 구현을 컴파일 단계에서 거부한 것이며 라이브러리 자체의 컴파일 실패를 검출로 세지 않았다. pub 필드는 Rust 소스 계약이 검출한다. 자동 변이는 실행하지 않았다. H-46 이후 자동 선택 범위는 기준 대비 바뀐 대상 src 파일과 그 diff 영역이다. 참조 그래프나 매니페스트 변경으로 crate 전체를 확장하지 않는다. 이번 변경의 application·domain src 파일이 대상이며 테스트·하네스 지원 입력의 보완은 H-46·H-47 인계에 따른다.


## 검사와 한계

새 생성자와 Rust 보호 테스트를 먼저 적용했을 때 이전 공개 생성자에서 E0308을 확인한 뒤 구현했다. 프로젝트·세션 훅 신뢰 적용 자체는 미확인이다. 실제 실행한 검사 결과와 Git 훅 결과를 증거로 남긴다. 하위 에이전트·독립 적대적 리뷰·비판은 실행하지 않으며 후속 리뷰는 메인이 수행한다. 포트 정의를 DB·키체인·macOS 실행 검증으로 주장하지 않는다.
