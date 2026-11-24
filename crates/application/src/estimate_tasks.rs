use crate::{AiRunId, EstimateCommitOutcome, SystemPortError, TrackingTarget};
use domain::{
    BaseProbability, CalibrationEstimateId, CommissionRateFact, CostEligibleTask,
    EstimateAssignmentError, EstimateAssignments, EstimateConsumption, EstimateContext,
    EstimateCostPeriod, EstimateDistance, EstimateTask, InstrumentId, InvestorProfile,
    IssuedEstimateTask, IssuedEstimateTaskId, Market, NoEstimateTask, NoThresholds, Price, Symbol,
    Timestamp, TradeCost, TradeCostError, TradeTaxFact, TradingDayHorizon, Volatility,
};
use std::future::Future;

pub trait EstimateAssignmentRepository: Send + Sync {
    fn load_assignments(
        &self,
    ) -> impl Future<Output = Result<EstimateAssignments<AiRunId>, SystemPortError>> + Send;
    fn commit_assignments(
        &self,
        previous: &EstimateAssignments<AiRunId>,
        next: &EstimateAssignments<AiRunId>,
    ) -> impl Future<Output = Result<EstimateCommitOutcome, SystemPortError>> + Send;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateEstimateTaskInput {
    pub task_id: CalibrationEstimateId,
    pub run: AiRunId,
    pub target: TrackingTarget,
    pub context: EstimateContext,
    pub volatility: Volatility,
    pub profile: InvestorProfile,
    pub commission: Option<CommissionRateFact>,
    pub tax: Option<TradeTaxFact>,
    pub maturity: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateEstimateTaskOutcome {
    Created(Box<IssuedEstimateTask<AiRunId>>),
    SkippedTask(NoEstimateTask),
    SkippedExcessiveCost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateEstimateTaskError {
    TargetMismatch,
    TradeCost(TradeCostError),
    Thresholds(NoThresholds),
    Assignment(EstimateAssignmentError),
    ConcurrentUpdate,
    Storage(SystemPortError),
}

#[derive(Debug)]
pub struct CreateEstimateTask<'a, Repository> {
    repository: &'a Repository,
}

impl<'a, Repository: EstimateAssignmentRepository> CreateEstimateTask<'a, Repository> {
    #[must_use]
    pub const fn new(repository: &'a Repository) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        input: CreateEstimateTaskInput,
    ) -> Result<CreateEstimateTaskOutcome, CreateEstimateTaskError> {
        if input.target.instrument() != input.volatility.instrument() {
            return Err(CreateEstimateTaskError::TargetMismatch);
        }
        let task = match EstimateTask::create(
            input.context,
            input.volatility,
            input.profile.horizon(),
            input.profile.hours_permission(),
            input.profile.risk_appetite(),
        ) {
            Ok(created) => created,
            Err(NoEstimateTask::InstrumentMismatch) => {
                return Err(CreateEstimateTaskError::TargetMismatch);
            }
            Err(reason) => return Ok(CreateEstimateTaskOutcome::SkippedTask(reason)),
        };
        let period = EstimateCostPeriod::from_calendar_result(&task, input.maturity)
            .map_err(CreateEstimateTaskError::TradeCost)?;
        let trade_cost = TradeCost::from_facts(input.commission, input.tax, period)
            .map_err(CreateEstimateTaskError::TradeCost)?;
        let eligible = match CostEligibleTask::assess(task, &trade_cost) {
            Ok(value) => value,
            Err(NoThresholds::ExcessiveCost) => {
                return Ok(CreateEstimateTaskOutcome::SkippedExcessiveCost);
            }
            Err(error) => return Err(CreateEstimateTaskError::Thresholds(error)),
        };
        let previous = self
            .repository
            .load_assignments()
            .await
            .map_err(CreateEstimateTaskError::Storage)?;
        let next = previous
            .issue(input.task_id.clone(), input.run, eligible)
            .map_err(CreateEstimateTaskError::Assignment)?;
        match self
            .repository
            .commit_assignments(&previous, &next)
            .await
            .map_err(CreateEstimateTaskError::Storage)?
        {
            EstimateCommitOutcome::Conflict => Err(CreateEstimateTaskError::ConcurrentUpdate),
            EstimateCommitOutcome::Committed => {
                let issued = next
                    .lookup(&input.task_id)
                    .map_err(CreateEstimateTaskError::Assignment)?;
                Ok(CreateEstimateTaskOutcome::Created(Box::new(issued.clone())))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetEstimateTaskInput {
    task_id: CalibrationEstimateId,
}

impl GetEstimateTaskInput {
    #[must_use]
    pub const fn new(task_id: CalibrationEstimateId) -> Self {
        Self { task_id }
    }

    #[must_use]
    pub const fn task_id(&self) -> &CalibrationEstimateId {
        &self.task_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EstimateTaskView {
    task_id: IssuedEstimateTaskId,
    instrument: InstrumentId,
    symbol: Symbol,
    market: Market,
    target: EstimateDistance,
    stop: EstimateDistance,
    horizon: TradingDayHorizon,
    base_probability: BaseProbability,
    reference_price: Price,
    asked_at: Timestamp,
    consumption: EstimateConsumption,
}

impl EstimateTaskView {
    #[must_use]
    pub fn from_issued(issued: &IssuedEstimateTask<AiRunId>) -> Self {
        let task = issued.task().task();
        let reference = task.reference();
        Self {
            task_id: issued.id().clone(),
            instrument: task.volatility().instrument().clone(),
            symbol: reference.instrument().symbol().clone(),
            market: reference.instrument().market(),
            target: task.target(),
            stop: task.stop(),
            horizon: task.horizon(),
            base_probability: task.base_probability(),
            reference_price: reference.price(),
            asked_at: task.created_at(),
            consumption: issued.consumption(),
        }
    }

    #[must_use]
    pub const fn task_id(&self) -> &IssuedEstimateTaskId {
        &self.task_id
    }

    #[must_use]
    pub const fn instrument(&self) -> &InstrumentId {
        &self.instrument
    }

    #[must_use]
    pub const fn symbol(&self) -> &Symbol {
        &self.symbol
    }

    #[must_use]
    pub const fn market(&self) -> &Market {
        &self.market
    }

    #[must_use]
    pub const fn target(&self) -> EstimateDistance {
        self.target
    }

    #[must_use]
    pub const fn stop(&self) -> EstimateDistance {
        self.stop
    }

    #[must_use]
    pub const fn horizon(&self) -> TradingDayHorizon {
        self.horizon
    }

    #[must_use]
    pub const fn base_probability(&self) -> BaseProbability {
        self.base_probability
    }

    #[must_use]
    pub const fn reference_price(&self) -> Price {
        self.reference_price
    }

    #[must_use]
    pub const fn asked_at(&self) -> Timestamp {
        self.asked_at
    }

    #[must_use]
    pub const fn consumption(&self) -> EstimateConsumption {
        self.consumption
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GetEstimateTaskError {
    UnknownTask,
    WrongRun,
    Storage(SystemPortError),
}

#[derive(Debug)]
pub struct GetEstimateTask<'a, Repository> {
    repository: &'a Repository,
}

impl<'a, Repository: EstimateAssignmentRepository> GetEstimateTask<'a, Repository> {
    #[must_use]
    pub const fn new(repository: &'a Repository) -> Self {
        Self { repository }
    }

    pub async fn execute(
        &self,
        run: &AiRunId,
        input: &GetEstimateTaskInput,
    ) -> Result<EstimateTaskView, GetEstimateTaskError> {
        let assignments = self
            .repository
            .load_assignments()
            .await
            .map_err(GetEstimateTaskError::Storage)?;
        let issued = assignments
            .lookup(input.task_id())
            .map_err(|_error| GetEstimateTaskError::UnknownTask)?;
        if issued.run() != run {
            return Err(GetEstimateTaskError::WrongRun);
        }
        Ok(EstimateTaskView::from_issued(issued))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CreateEstimateTask, CreateEstimateTaskError, CreateEstimateTaskInput,
        CreateEstimateTaskOutcome, EstimateAssignmentRepository, GetEstimateTask,
        GetEstimateTaskError, GetEstimateTaskInput,
    };
    use crate::{
        AiRunId, EstimateCommitOutcome, SystemFailure, SystemPort, SystemPortError, TrackingTarget,
    };
    use domain::{
        AdjustmentBasis, CalibrationEstimateId, Candle, CandleObservation, CandlePeriod,
        CandleStatus, CandleTime, CommissionRateFact, CommissionValidity, CommonShare, CostRate,
        Currency, DisplayInstrumentName, EstimateAssignmentError, EstimateAssignments,
        EstimateConsumption, EstimateContext, InstrumentId, InvestorProfile, KoreanMasterDetail,
        LiquidationTrading, ListingStatus, Market, MasterCode, MasterInstrument,
        MasterInstrumentRow, MasterSecurityType, Nanosecond, NoEstimateTask, NumericInput,
        NxtTrading, OffsetSeconds, Ohlcv, Price, PriceAdjustment, Quantity, Ratio, Symbol,
        Timestamp, TradeTaxFact, TradingSuspension, UnixSeconds, Volatility,
    };
    use std::future::Future;
    use std::sync::RwLock;
    use std::task::{Context, Poll, Waker};

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("동기 테스트 퓨처"),
        }
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    fn day(index: i64) -> Timestamp {
        at(index.checked_mul(86_400).expect("일"))
    }

    fn local(time: Timestamp) -> Timestamp {
        Timestamp::from_parts(
            time.utc_seconds(),
            time.nanosecond(),
            OffsetSeconds::new(32_400),
        )
        .expect("현지 시각")
    }

    fn local_midnight(index: i64) -> Timestamp {
        local(at(day(index)
            .utc_seconds()
            .as_i64()
            .checked_sub(32_400)
            .expect("현지 자정")))
    }

    fn price(text: &str) -> Price {
        Price::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("가격")
    }

    fn quantity(text: &str) -> Quantity {
        Quantity::parse(&NumericInput::try_new(text).expect("입력")).expect("수량")
    }

    fn ratio(text: &str) -> Ratio {
        Ratio::parse(&NumericInput::try_new(text).expect("입력")).expect("비율")
    }

    fn master(id_text: &str, symbol_text: &str) -> MasterInstrument {
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("시장"),
            instrument_id: InstrumentId::try_new(id_text).expect("ID"),
            symbol: Symbol::try_new(symbol_text).expect("심볼"),
            name: DisplayInstrumentName::try_new("샘플 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Unsupported,
            }),
            observed_at: day(60),
        })
        .expect("마스터")
    }

    fn volatility_series(source: &MasterInstrument, count: i64, high: &str) -> Volatility {
        let candles: Vec<_> = (0_i64..count)
            .map(|index| {
                let value = price(if index % 2 == 0 { "100" } else { high });
                Candle::try_new(
                    source.row().instrument_id.clone(),
                    CandleTime::try_new(local_midnight(index), CandlePeriod::Daily)
                        .expect("일봉 시각"),
                    PriceAdjustment::Adjusted(AdjustmentBasis::new(day(count))),
                    Ohlcv::try_new(value, value, value, value, quantity("10")).expect("OHLCV"),
                    CandleObservation::new(day(count), CandleStatus::Finalized),
                )
                .expect("일봉")
            })
            .collect();
        Volatility::calculate(&candles).expect("변동성")
    }

    #[derive(Default)]
    struct MemoryTaskRepo {
        assignments: RwLock<EstimateAssignments<AiRunId>>,
        conflict: RwLock<bool>,
    }

    impl EstimateAssignmentRepository for MemoryTaskRepo {
        fn load_assignments(
            &self,
        ) -> impl Future<Output = Result<EstimateAssignments<AiRunId>, SystemPortError>> + Send
        {
            let err =
                SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity);
            let result = match self.assignments.read() {
                Ok(guard) => Ok(guard.clone()),
                Err(_) => Err(err),
            };
            std::future::ready(result)
        }

        fn commit_assignments(
            &self,
            previous: &EstimateAssignments<AiRunId>,
            next: &EstimateAssignments<AiRunId>,
        ) -> impl Future<Output = Result<EstimateCommitOutcome, SystemPortError>> + Send {
            let err =
                SystemPortError::new(SystemPort::Repository, SystemFailure::RepositoryIntegrity);
            let result = match (self.conflict.read(), self.assignments.write()) {
                (Ok(flag), _) if *flag => Ok(EstimateCommitOutcome::Conflict),
                (Ok(_), Ok(mut guard)) if &*guard == previous => {
                    *guard = next.clone();
                    Ok(EstimateCommitOutcome::Committed)
                }
                (Ok(_), Ok(_)) => Ok(EstimateCommitOutcome::Conflict),
                (Err(_), _) | (Ok(_), Err(_)) => Err(err),
            };
            std::future::ready(result)
        }
    }

    fn base_input(
        task_id: &str,
        run_id: &str,
        source: &MasterInstrument,
        sample_candles: i64,
        cost_rate: &str,
    ) -> CreateEstimateTaskInput {
        let last_day = sample_candles.checked_sub(1).expect("마지막 날");
        let rate = CostRate::try_new(ratio(cost_rate)).expect("요율");
        CreateEstimateTaskInput {
            task_id: CalibrationEstimateId::try_new(task_id).expect("과제 ID"),
            run: AiRunId::try_new(run_id).expect("실행 ID"),
            target: TrackingTarget::for_watchlist(source.row().instrument_id.clone()),
            context: EstimateContext::new(
                source
                    .price_observation(price("100"), local(day(last_day)))
                    .expect("기준가"),
                day(last_day),
            ),
            volatility: volatility_series(source, sample_candles, "102"),
            profile: InvestorProfile::default(),
            commission: Some(CommissionRateFact::from_commissions_result(
                Market::Kr,
                rate,
                CommissionValidity::try_new(None, None).expect("유효기간"),
            )),
            tax: Some(TradeTaxFact::from_schedule_result(
                Market::Kr,
                CostRate::try_new(ratio("0")).expect("매수세"),
                CostRate::try_new(ratio("0")).expect("매도세"),
            )),
            maturity: day(last_day.checked_add(60).expect("만기")),
        }
    }

    #[test]
    fn creates_and_reads_code_defined_task_by_task_id_only() {
        let repo = MemoryTaskRepo::default();
        let creator = CreateEstimateTask::new(&repo);
        let reader = GetEstimateTask::new(&repo);
        let source = master("inst-005930", "005930");
        let input = base_input("task-1", "run-1", &source, 61, "0.00015");
        let expected_asked_at = day(60);

        let created = match run(creator.execute(input)).expect("생성") {
            CreateEstimateTaskOutcome::Created(issued) => issued,
            CreateEstimateTaskOutcome::SkippedTask(_)
            | CreateEstimateTaskOutcome::SkippedExcessiveCost => panic!("생성 기대"),
        };

        let run_one = AiRunId::try_new("run-1").expect("실행");
        let lookup_input =
            GetEstimateTaskInput::new(CalibrationEstimateId::try_new("task-1").expect("ID"));
        let view = run(reader.execute(&run_one, &lookup_input)).expect("조회");
        assert_eq!(view.task_id(), created.id());
        assert_eq!(view.instrument(), &source.row().instrument_id);
        assert_eq!(view.symbol(), &source.row().symbol);
        assert_eq!(view.market(), &Market::Kr);
        assert_eq!(view.target(), created.task().task().target());
        assert_eq!(view.stop(), created.task().task().stop());
        assert_eq!(view.horizon(), created.task().task().horizon());
        assert_eq!(
            view.base_probability(),
            created.task().task().base_probability()
        );
        assert_eq!(view.reference_price(), price("100"));
        assert_eq!(view.asked_at(), expected_asked_at);
        assert_eq!(view.consumption(), EstimateConsumption::Available);

        let other_run = AiRunId::try_new("run-2").expect("다른 실행");
        assert_eq!(
            run(reader.execute(&other_run, &lookup_input)),
            Err(GetEstimateTaskError::WrongRun)
        );
        let missing_input =
            GetEstimateTaskInput::new(CalibrationEstimateId::try_new("unknown").expect("미지 ID"));
        assert_eq!(
            run(reader.execute(&run_one, &missing_input)),
            Err(GetEstimateTaskError::UnknownTask)
        );
    }

    #[test]
    fn skips_when_samples_under_60_or_cost_ratio_exceeds_5_pp() {
        let repo = MemoryTaskRepo::default();
        let creator = CreateEstimateTask::new(&repo);
        let source = master("inst-short", "005930");

        let short_input = base_input("task-short", "run-1", &source, 60, "0.00015");
        assert_eq!(
            run(creator.execute(short_input)),
            Ok(CreateEstimateTaskOutcome::SkippedTask(
                NoEstimateTask::InsufficientSamples
            ))
        );

        let expensive_input = base_input("task-cost", "run-1", &source, 61, "0.10");
        assert_eq!(
            run(creator.execute(expensive_input)),
            Ok(CreateEstimateTaskOutcome::SkippedExcessiveCost)
        );

        let stored = run(repo.load_assignments()).expect("로드");
        assert_eq!(
            stored
                .assigned(&AiRunId::try_new("run-1").expect("실행"))
                .len(),
            0
        );
    }

    #[test]
    fn rejects_target_mismatch_duplicate_task_id_and_conflict() {
        let repo = MemoryTaskRepo::default();
        let creator = CreateEstimateTask::new(&repo);
        let source = master("inst-a", "005930");

        let mut mismatch = base_input("task-mismatch", "run-1", &source, 61, "0.00015");
        mismatch.target = TrackingTarget::for_holding(InstrumentId::try_new("inst-b").expect("B"));
        assert_eq!(
            run(creator.execute(mismatch)),
            Err(CreateEstimateTaskError::TargetMismatch)
        );

        let valid = base_input("task-dup", "run-1", &source, 61, "0.00015");
        assert!(matches_created(
            &run(creator.execute(valid.clone())).expect("첫 생성")
        ));
        assert_eq!(
            run(creator.execute(valid)),
            Err(CreateEstimateTaskError::Assignment(
                EstimateAssignmentError::AlreadyIssued
            ))
        );

        *repo.conflict.write().expect("충돌") = true;
        let conflicted = base_input("task-conflict", "run-1", &source, 61, "0.00015");
        assert_eq!(
            run(creator.execute(conflicted)),
            Err(CreateEstimateTaskError::ConcurrentUpdate)
        );
    }

    fn matches_created(outcome: &CreateEstimateTaskOutcome) -> bool {
        match outcome {
            CreateEstimateTaskOutcome::Created(_) => true,
            CreateEstimateTaskOutcome::SkippedTask(_)
            | CreateEstimateTaskOutcome::SkippedExcessiveCost => false,
        }
    }
}
