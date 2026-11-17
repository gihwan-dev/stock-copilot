# A-24 빠진 평가일 연결

- 상태: 초안

## 결정

A-16의 기존 관측·저장 동작과 테스트를 유지한다. 새 PlanValuationGap은 Clock을 한 번 읽고 ValuationCalendarSource.days(last, now)로 받은 자료를 ValuationGapPlan::new에 전달한다. 날짜·금액 계산은 하지 않는다. 달력 포트는 (last, KST 오늘]의 모든 날짜에 한국/미국 달력 또는 날짜별 unavailable 사유를 순서대로 공급한다. 날짜 매핑과 출처는 J-11 책임이며, 전역 공급 실패는 원래 PortError로 반환한다. 확인된 양국 휴장과 획득 실패의 구별은 기존 domain에 맡긴다.

계획은 저장 허가가 아니다. 이번 구현은 호출 가능한 후보 계획 유스케이스까지이며 자동 재계산·저장은 정의 대기다. 계획을 성공 기록이나 중복 방지 표식으로 저장하지 않으며 이벤트를 발행하지 않는다. 기존 TakeValuationSnapshot을 과거 Clock으로 호출하지 않는다. 과거 시각을 만들어 현재 관측에 붙이지 않는다.

## 기준별 충족과 정의 대기

| 작업 기준 | 상태 | 근거 또는 빠진 계약 |
|---|---|---|
| 누락일 계획 유스케이스·달력 포트 | 충족 | PlanValuationGap, 여러 후보·휴장·실패·KST 자정 통합 테스트 |
| 실제 시작 조립·last 선택과 확보 | 정의 대기 | J-11 시작 호출 및 계좌별 이력 조회 범위·최초 기록 기준일·실패일 재시도를 포함한 last 선택 정책과 연결이 없음 |
| 달력 실패와 확인 휴장 구별 | 충족 | 전역 오류는 Calendar(PortError) 원값, 날짜별 오류는 domain 계획에 보존 |
| 날짜·금액 계산은 domain, 현재 시각은 Clock | 충족 | 기존 ValuationGapPlan::new 호출, 시각 한 번 샘플링 |
| 일봉과 거래내역으로 모두 채움 / 일봉 없는 날만 실패 | 정의 대기 | SnapshotInstrumentHistory의 종목/행사 이력, 대조된 SnapshotLedgers 공급 포트가 없음. 기존 MarketDataRepository.candles와 TransactionRepository.latest만으로 이를 대체하지 않음 |
| 과거 날짜 16:00 KST 평가 | 정의 대기 | CalendarDate에서 해당 날짜 16:00 KST Timestamp를 만드는 domain 공개 함수가 없음. application 산술이나 새 domain 함수 추가는 브리프가 금지 |
| 가격 근거와 실제 재계산 입력 동일성 / 통화·날짜 거부 | 정의 대기 | ValuationPriceEvidence::closing_price는 존재하지만 위 이력/시각 공급이 없어 실행 연결하지 않음. D-48 테스트 통과를 A-24 구현으로 세지 않음 |
| 없음과 0 구분 | 정의 대기 | 계획 단계에는 금액 입력이 없어 해당 없음. 재계산 입력에서 없음/명시적 0/완전성을 구별하는 연결은 정의 대기 |
| 날짜별 성공·실패 영속 저장 | 정의 대기 | 최소 ValuationDayRepository::record 경계만 정의. S-18 구현과 성공일 재조회/재시도 정책은 정의 대기 |
| 중복 실행 시 이미 채운 날 재생성 금지 | 정의 대기 | ValuationRepository.history는 TimeInterval만 받음. 날짜별 성공 조회/누락 계획 재연결과 실패 재시도 계약이 없음. 계획은 매번 반환하되 스냅샷 생성은 하지 않음 |

필요한 domain 함수나 포트 메서드가 없으면 만들지 말라는 구체 지시를 따른다. 이 결과는 A-24 전체 수용 기준의 완료가 아니다. 정의 대기 항목을 휴장·0원·성공값으로 위장하지 않는다.

## 새 포트

ValuationCalendarSource::days(last: CalendarDate, now: Timestamp) -> Future<Result<Vec<ValuationCalendarDay>, PortError>>. 달력 경계 추가의 명시적 승인을 사용했다. 실제 공급 구현 없이 tests/valuation_gap.rs의 Script 기반 가짜로 검증한다. 계좌에 무관한 달력 공급이며 last는 호출자가 확인한 마지막 평가일이다. 최초 기록이 없는 계좌의 기준일 선택도 별도 정책이며 임의로 정하지 않는다.

ValuationDayRepository::record(account, &ValuationDayResult)는 날짜별 성공·실패 결과를 저장할 최소 포트다. 기존 RecordOutcome으로 같은 결과의 중복을 표현한다. 저장 구현·호출은 하지 않으며 실패 뒤 성공 갱신, 동일 날짜 다른 성공값, 동시 실행 정책은 S-18이 정한다. 마지막 성공일 조회 메서드는 발명하지 않는다.

달력 공급의 전역 실패는 기존 읽기 포트 오류 PortError로 전달한다. 공급자가 준 endpoint·HTTP 상태·정규화 원인·재시도 정보를 그대로 보존하며 저장 데이터 손상으로 변환하지 않는다. 실제 달력 조회 공급자가 ReadEndpoint::Calendar와 응답 원인을 올바르게 결합할 책임이 있다. 타입이나 유스케이스가 endpoint를 Calendar로 강제·재작성하지 않으며 어댑터 계약 테스트에서 대조한다. 공급자는 외부 조회 실패를 Repository 오류로 위장하지 않는다. 저장된 달력을 읽는 별도 공급 구현이 필요하면 그 저장 오류와 조회 오류의 합성 계약부터 확정한다. 현재 공급 구현은 없다.

S-18의 날짜 결과 저장은 기존 ValuationRepository와 별도의 성공 정본을 만들라는 뜻이 아니다. 성공 스냅샷은 같은 계좌의 기존 history 조회에 나타나야 한다. 성공 정본과 날짜 결과가 일관돼야 한다는 조회 계약만 인계한다. 구현 객체·테이블·물리적 저장 구조를 여기서 정하지 않으며, 성공 저장과 날짜 결과 기록의 원자성·중복·실패 뒤 성공 정책을 S-18에서 정한다. A-24 후속은 확정된 계약의 저장 성공 뒤에만 기존 계좌 이벤트를 발행한다. 현재는 어느 저장 포트도 호출하지 않는다.

unavailable(..., ClosedDay)는 ADR0118대로 계획에 남는 획득 실패다. 확인된 양국 휴장은 계획에서 제외된다. ClosedDay 사유 하나만 읽어 확인 휴장 화면 문구로 바꾸지 않는다. 상세 획득 원인 보존·화면 문구 연결은 S-18/J-11 인계이며 현재 저장·표시 경로는 없다.

## 인계

| 받는 작업 | 내용 |
|---|---|
| D-31/D-48 | 평가일에서 16:00 KST를 만드는 공개 domain 함수 |
| A-24 후속 / J-11 | 과거 종목·기업행사·대조 현금 원장 공급의 포트 계약, 달력 실제 매핑·시작 조립 |
| S-18 | 계좌/평가일별 성공·실패 기록, 성공일 조회·재시도·동시 중복 정책, 기존 history의 성공 정본과 날짜 결과의 원자성 |
| A-24 후속 | 실패일을 포함하는 재시도 하한과 이미 성공한 날짜의 조회/재사용을 결합할 것. 위 계약 후 일봉·거래내역 재계산, 가격 증거 동일성, 날짜별 결과 저장과 기존 A-16 이벤트 연결 |

## 검증 경계

frozen install과 lefthook install 성공. 프로젝트 신뢰·세션 훅 신뢰/적용은 확인 불가이며 홈 설정·인증 파일을 읽거나 바꾸지 않았다. 실제 키·계좌·API 없음. 새 외부 의존·개발 의존·가짜 공용 메서드·기존 테스트 변경·검사 끄기 없음. 최초 새 테스트는 공개 API 부재 E0432로 실패했으며 구현 후 집중 테스트4개 통과. 이후 자체 테스트 fixture 수정은 기존 master 테스트 변경에 해당하지 않는다.

## 검사 보완

새 테스트 파일의 cfg(test) 모듈 누락과 숫자 형식·검사된 산술·변수 이름을 정상 Clippy 결과에 따라 수정했다. 기존 master 테스트의 단언·준비는 변경하지 않았다. 승인 test-diff 변경0건이다. 보호 변경은 새 ADR을 등록한 공식 .codex/config.toml·Claude 훅 해시 생성물이며 검사 범위를 완화하지 않는다.

변경분 자동 변이는 최초 실행에서 선택1개·컴파일불가1개·검출0개·생존0개였다. ValuationGapPlan에 Default가 없어 성공 기본값 변형이 컴파일되지 않았다. 이 결과를 테스트가 변형을 잡았다는 증거로 세지 않는다.

## 손 변형

mktemp 복사본에서 원본 대조군4개, 오류 전달 변형20개, 원복 대조군4개를 실제 실행했다. 컴파일20·검출20·생존0·컴파일불가0이다. 이 결과는 계획 유스케이스의 오류 전달만 검증하며 정의 대기인 재계산·저장 경로의 증거가 아니다. 로그와 원복 해시 일치는 target/A-24/hand-*.log·hand-results.json·hand-proof.json에 있다.

| 번호 | 바꾼 것 | 잡은 테스트 |
|---|---|---|
| 1 | Calendar 오류를 MissingPrice로 치환 | source_failures_are_not_relabelled_as_holidays, source_failures_are_not_relabelled_as_holidays |
| 2 | Calendar 오류를 InvalidCandle로 치환 | source_failures_are_not_relabelled_as_holidays, source_failures_are_not_relabelled_as_holidays |
| 3 | Calendar 오류를 MissingFx로 치환 | source_failures_are_not_relabelled_as_holidays, source_failures_are_not_relabelled_as_holidays |
| 4 | Calendar 오류를 InvalidFx로 치환 | source_failures_are_not_relabelled_as_holidays, source_failures_are_not_relabelled_as_holidays |
| 5 | Calendar 오류를 MissingCash로 치환 | source_failures_are_not_relabelled_as_holidays, source_failures_are_not_relabelled_as_holidays |
| 6 | Calendar 오류를 IncompleteValuation로 치환 | source_failures_are_not_relabelled_as_holidays, source_failures_are_not_relabelled_as_holidays |
| 7 | Calendar 오류를 MissingPrevious로 치환 | source_failures_are_not_relabelled_as_holidays, source_failures_are_not_relabelled_as_holidays |
| 8 | Calendar 오류를 MissingFlows로 치환 | source_failures_are_not_relabelled_as_holidays, source_failures_are_not_relabelled_as_holidays |
| 9 | Calendar 오류를 MissingHistory로 치환 | source_failures_are_not_relabelled_as_holidays, source_failures_are_not_relabelled_as_holidays |
| 10 | Calendar 오류를 ClosedDay로 치환 | source_failures_are_not_relabelled_as_holidays, source_failures_are_not_relabelled_as_holidays |
| 11 | Plan 오류를 MissingPrice로 치환 | incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain, incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain |
| 12 | Plan 오류를 InvalidCandle로 치환 | incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain, incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain |
| 13 | Plan 오류를 MissingFx로 치환 | incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain, incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain |
| 14 | Plan 오류를 InvalidFx로 치환 | incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain, incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain |
| 15 | Plan 오류를 MissingCash로 치환 | incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain, incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain |
| 16 | Plan 오류를 IncompleteValuation로 치환 | incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain, incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain |
| 17 | Plan 오류를 MissingPrevious로 치환 | incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain, incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain |
| 18 | Plan 오류를 MissingFlows로 치환 | incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain, incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain |
| 19 | Plan 오류를 MissingHistory로 치환 | incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain, incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain |
| 20 | Plan 오류를 ClosedDay로 치환 | incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain, incomplete_calendar_ranges_and_future_last_dates_are_rejected_by_domain |
