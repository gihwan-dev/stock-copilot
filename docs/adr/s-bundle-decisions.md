# S 묶음 설계 결정 기록

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| S-18 | 도메인(`Volatility`, `EstimateTask`, `CostEligibleTask`, `EstimateAssignments`, `EstimateOutcome`, `CalibrationFact`, `LotBook`, `ConfirmedFlowPeriod`, `QuoteInstrument`, `ValuationSnapshot`, `ValuationDayResult`)에 기존 `SavedInvestorProfile` 선례와 같은 검증형 `Saved*` + `saved_state()`/`restore_from_saved_state()`를 추가하되 `ResolvedOrderInstrument` 등 봉인 생성자는 공개하지 않음 | `store-sqlite`가 원본 일봉·거래내역 전체를 매번 재실행해 복원하거나 도메인 필드를 공개 |
| S-18 | 과제 발급 영속화를 위해 `EstimateTaskRepository::issue_task` 포트를 추가하고 `ValuationDayRepository` 기존 시그니처는 유지 | 기존 `EstimateSubmissionRepository`·`ValuationDayRepository` 시그니처를 깨고 메서드를 변경 |
| S-18 | 추정 과제·제출·판정·보정표·일별 평가를 단일 마이그레이션 `0010_estimates_and_valuations.sql`에 STRICT 테이블과 수정·삭제·덮어쓰기 차단 트리거로 배치하고, 중첩 구조는 `thesis` 선례와 같은 길이 접두 정규 코덱으로 저장 | `serde_json` 의존성 추가 또는 수십 개 조인 테이블로 정규화 |
| S-22 | `ProposalLifetime::as_seconds`와 `CollectedItem::from_saved_source` 최소 접근자·복원자만 도메인에 추가하고, `Proposal<Validated>`·`ResolvedOrderInstrument`·`IssuedEstimateTaskId` 봉인 경계는 유지 | `store-sqlite`가 초안 수명·수집 출처를 손실 저장하거나 도메인 내부 필드를 공개 |
| S-22 | `SignalStore`가 `SignalRepository`, `SignalInvalidationRepository`, `ProposalRepository`를 단일 SQLite DB 위에서 구현하고 `estimate_submission_records`·`estimate_calibrations`와 직접 연결하며, `proposal_drafts`는 삭제 차단·`current`→`superseded` 전이만 허용하는 트리거와 만료 시각·오프셋 보존 컬럼으로 저장 | 초안 저장소와 신호 저장소를 별도 DB·포트 구조로 분리하거나 초안을 덮어쓰기 업데이트 |
