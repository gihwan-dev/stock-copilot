use application::resolve_estimates::{
    EstimateResolutionRepository, ResolutionCalibration, ResolutionCommitOutcome,
    ResolutionDataIssue,
};
use application::{
    AiRunId, EstimateCommitOutcome, EstimateRepository, EstimateSubmissionRepository,
    EstimateTaskRepository, ReasoningEffort, RecordOutcome, SystemFailure, ValuationDayRepository,
};
use domain::{
    AccountId, Calibration, CalibrationEstimateId, CalibrationPosition, CostEligibleTask,
    EstimateMetadata, EstimateNarrative, EstimateOutcome, EstimateSubmissionRecord,
    OutcomeUnavailable, SignalSubmission, ValuationDayResult,
};

#[derive(Debug, Clone)]
pub struct EstimateLifecycleFixture {
    pub task_id: CalibrationEstimateId,
    pub run_id: AiRunId,
    pub other_run_id: AiRunId,
    pub task: CostEligibleTask,
    pub submission: SignalSubmission,
    pub narrative: EstimateNarrative,
    pub metadata: EstimateMetadata<ReasoningEffort>,
    pub outcome: EstimateOutcome,
    pub other_outcome: EstimateOutcome,
}

async fn verify_task_issuance_and_submission<P>(
    port: &P,
    fixture: &EstimateLifecycleFixture,
) -> Option<EstimateSubmissionRecord<AiRunId, ReasoningEffort>>
where
    P: EstimateTaskRepository + EstimateSubmissionRepository + EstimateResolutionRepository,
{
    assert_eq!(
        port.issue_task(&fixture.task_id, &fixture.run_id, &fixture.task)
            .await,
        Ok(RecordOutcome::Recorded),
        "최초 과제 발급 기록"
    );
    assert_eq!(
        port.issue_task(&fixture.task_id, &fixture.run_id, &fixture.task)
            .await,
        Ok(RecordOutcome::AlreadyRecorded),
        "동일 과제 중복 발급은 멱등"
    );
    let conflicting = port
        .issue_task(&fixture.task_id, &fixture.other_run_id, &fixture.task)
        .await;
    assert_eq!(
        conflicting.as_ref().err().map(|err| err.failure()),
        Some(SystemFailure::RepositoryIntegrity),
        "발급된 과제 변경 거부"
    );
    let assignments_res = port.assignments().await;
    assert!(assignments_res.is_ok(), "배정 조회 성공");
    let Ok(assignments) = assignments_res else {
        return None;
    };
    let issued_res = assignments.lookup(&fixture.task_id);
    assert!(issued_res.is_ok(), "발급 과제 존재");
    let Ok(issued) = issued_res else {
        return None;
    };
    let consumed_res = assignments.consume(&fixture.run_id, std::slice::from_ref(&fixture.task_id));
    assert!(consumed_res.is_ok(), "과제 소비 성공");
    let Ok(consumed) = consumed_res else {
        return None;
    };
    let record_res = EstimateSubmissionRecord::create(
        issued,
        fixture.submission.clone(),
        fixture.narrative.clone(),
        fixture.metadata.clone(),
    );
    assert!(record_res.is_ok(), "제출 기록 생성 성공");
    let Ok(record) = record_res else {
        return None;
    };
    verify_submission_commit(port, fixture, &assignments, &consumed, &record).await;
    Some(record)
}

async fn verify_submission_commit<P>(
    port: &P,
    fixture: &EstimateLifecycleFixture,
    assignments: &domain::EstimateAssignments<AiRunId>,
    consumed: &domain::EstimateAssignments<AiRunId>,
    record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
) where
    P: EstimateSubmissionRepository + EstimateResolutionRepository,
{
    assert_eq!(
        port.commit_submission(assignments, consumed, std::slice::from_ref(record))
            .await,
        Ok(EstimateCommitOutcome::Committed),
        "최초 제출 커밋"
    );
    assert_eq!(
        port.commit_submission(assignments, consumed, std::slice::from_ref(record))
            .await,
        Ok(EstimateCommitOutcome::Conflict),
        "이전 배정 스냅숏 재커밋 거부"
    );
    let instrument = fixture.task.task().volatility().instrument();
    assert_eq!(
        port.submission_records(instrument).await,
        Ok(vec![record.clone()]),
        "종목별 제출 기록 조회"
    );
    assert_eq!(
        port.pending_submissions().await,
        Ok(vec![record.clone()]),
        "판정 전 대기 제출 목록"
    );
}

async fn prepare_initial_calibration<P>(
    port: &P,
    key: &domain::CalibrationKey,
) -> Option<ResolutionCalibration>
where
    P: EstimateRepository + EstimateResolutionRepository,
{
    let initial_res = Calibration::try_new(key.clone(), &[]);
    assert!(initial_res.is_ok(), "빈 보정표 생성");
    let Ok(initial_cal) = initial_res else {
        return None;
    };
    assert_eq!(
        EstimateRepository::append_calibration(port, key, &initial_cal).await,
        Ok(RecordOutcome::Recorded),
        "초기 보정표 기록"
    );
    let before_res = EstimateResolutionRepository::calibration(port, key).await;
    assert!(
        before_res.as_ref().is_ok_and(Option::is_some),
        "초기 보정표 조회"
    );
    let Ok(Some(before)) = before_res else {
        return None;
    };
    assert_eq!(
        before.head.position(),
        CalibrationPosition::new(0),
        "초기 위치 0"
    );
    Some(before)
}

async fn verify_calibration_and_resolution<P>(
    port: &P,
    fixture: &EstimateLifecycleFixture,
    record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
) -> Option<(ResolutionCalibration, Calibration)>
where
    P: EstimateRepository + EstimateResolutionRepository,
{
    let key = fixture.submission.calibration_key();
    let before = prepare_initial_calibration(port, key).await?;
    assert_eq!(
        port.record_unavailable(
            record,
            ResolutionDataIssue::Incomplete(OutcomeUnavailable::MissingCandles)
        )
        .await,
        Ok(ResolutionCommitOutcome::Committed),
        "판정 전 결측 기록 허용"
    );
    let fact_opt = fixture.outcome.calibration_fact(
        fixture.task_id.clone(),
        key.model().clone(),
        fixture.submission.probability(),
    );
    assert!(fact_opt.is_some(), "보정 사실 생성");
    let fact = fact_opt?;
    let next_res = before.snapshot.calibration().with_fact(fact);
    assert!(next_res.is_ok(), "보정표 갱신 생성");
    let Ok(next_cal) = next_res else {
        return None;
    };
    assert_eq!(
        port.commit_resolution(record, &fixture.outcome, &before, &next_cal)
            .await,
        Ok(ResolutionCommitOutcome::Committed),
        "판정·보정표 원자 커밋"
    );
    let after_res = EstimateResolutionRepository::calibration(port, key).await;
    assert!(
        after_res.as_ref().is_ok_and(Option::is_some),
        "갱신 보정표 조회"
    );
    let Ok(Some(after)) = after_res else {
        return None;
    };
    Some((after, next_cal))
}

async fn verify_resolved_immutability<P>(
    port: &P,
    fixture: &EstimateLifecycleFixture,
    record: &EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
    after: &ResolutionCalibration,
    next_cal: &Calibration,
) where
    P: EstimateResolutionRepository,
{
    assert_eq!(
        port.pending_submissions().await,
        Ok(Vec::new()),
        "판정 후 대기 목록 비움"
    );
    assert_eq!(
        after.head.position(),
        CalibrationPosition::new(1),
        "보정 위치 1 증가"
    );
    assert_eq!(after.snapshot.calibration(), next_cal, "갱신 보정표 일치");
    assert_eq!(
        port.commit_resolution(record, &fixture.other_outcome, after, next_cal)
            .await,
        Ok(ResolutionCommitOutcome::AlreadyResolved),
        "이미 판정된 추정 재판정 거부"
    );
    assert_eq!(
        port.record_unavailable(
            record,
            ResolutionDataIssue::Incomplete(OutcomeUnavailable::Calendar)
        )
        .await,
        Ok(ResolutionCommitOutcome::AlreadyResolved),
        "판정 완료 뒤 결측 기록 거부"
    );
    assert_eq!(
        EstimateResolutionRepository::resolution(port, &fixture.task_id).await,
        Ok(Some(fixture.outcome.clone())),
        "재판정 시도 뒤 원래 판정 보존"
    );
}

pub async fn estimate_task_submission_resolution_lifecycle<P>(
    port: &P,
    fixture: &EstimateLifecycleFixture,
) where
    P: EstimateTaskRepository
        + EstimateRepository
        + EstimateSubmissionRepository
        + EstimateResolutionRepository,
{
    assert_ne!(
        fixture.outcome, fixture.other_outcome,
        "서로 다른 판정 결과 증인"
    );
    let record = verify_task_issuance_and_submission(port, fixture).await;
    assert!(record.is_some(), "제출 단계 완료");
    if let Some(submitted) = record {
        let resolved = verify_calibration_and_resolution(port, fixture, &submitted).await;
        assert!(resolved.is_some(), "판정 단계 완료");
        if let Some((after, next_cal)) = resolved {
            verify_resolved_immutability(port, fixture, &submitted, &after, &next_cal).await;
        }
    }
}

pub async fn valuation_day_append_only<P: ValuationDayRepository>(
    port: &P,
    accounts: (&AccountId, &AccountId),
    first: &ValuationDayResult,
    conflicting: &ValuationDayResult,
) {
    let (account, other) = accounts;
    assert_ne!(account, other, "서로 다른 계좌 증인");
    assert_eq!(first.date(), conflicting.date(), "같은 평가일 증인");
    assert_ne!(first, conflicting, "서로 다른 평가 결과 증인");
    assert_eq!(
        port.record(account, first).await,
        Ok(RecordOutcome::Recorded),
        "최초 일별 평가 기록"
    );
    assert_eq!(
        port.record(account, first).await,
        Ok(RecordOutcome::AlreadyRecorded),
        "동일 일별 평가 재기록 멱등"
    );
    let rejected = port.record(account, conflicting).await;
    assert_eq!(
        rejected.as_ref().err().map(|err| err.failure()),
        Some(SystemFailure::RepositoryIntegrity),
        "확정된 일별 평가 변경 거부"
    );
    assert_eq!(
        port.record(other, conflicting).await,
        Ok(RecordOutcome::Recorded),
        "다른 계좌는 독립 기록"
    );
}
