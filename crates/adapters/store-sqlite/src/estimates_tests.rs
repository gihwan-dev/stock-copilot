use crate::sql::{estimates as sql, estimates_tests as test_sql};
use crate::tests::{Fixture, clock, key, runtime, timestamp};
use crate::{Database, DatabasePath, EstimateStore, ValuationStore};
use application::resolve_estimates::{
    EstimateResolutionRepository, ResolutionCommitOutcome, ResolutionDataIssue, ResolutionInputs,
    ResolutionMarketData, ResolutionStatus, ResolveEstimates,
};
use application::{
    AiRunId, EstimateRepository, EstimateTaskRepository, EventBusBuilder, ReasoningEffort,
    RecordOutcome, SubmitEstimate, SystemFailure, ValuationDayRepository,
};
use domain::{
    AccountId, AdjustmentBasis, AiProbability, AiProbabilityInput, BuyingPower, CalendarDate,
    CalendarYear, Calibration, CalibrationEstimateId, CalibrationKey, CalibrationModel, Candle,
    CandleObservation, CandlePeriod, CandleStatus, CandleTime, CommissionRateFact,
    CommissionValidity, CommonShare, CostEligibleTask, CostRate, Currency, DayNumber,
    DisplayInstrumentName, EstimateContext, EstimateCostPeriod, EstimateMetadata,
    EstimateNarrative, EstimateOutcome, EstimatePrompt, EstimateTask, EvidenceBundleId, Horizon,
    HorizonSelection, HoursPermission, InstrumentId, KoreanMasterDetail, LiquidationTrading,
    ListingStatus, Market, MarketCalendar, MarketSession, MasterCode, MasterInstrument,
    MasterInstrumentRow, MasterSecurityType, ModelIdentityPart, Money, MonthNumber, Nanosecond,
    NumericInput, NxtTrading, OffsetSeconds, Ohlcv, OutcomeAdjustment, OutcomeState, Portfolio,
    Price, PriceAdjustment, PromptHash, Quantity, Ratio, RiskAppetite, ScorecardPromptId,
    SessionData, SessionInformation, SignalSubmission, SignalTaskKey, SnapshotCalendar,
    SnapshotError, Symbol, TimeInterval, Timestamp, TradeCost, TradeTaxFact, TradingSuspension,
    UnixSeconds, UntrustedText, ValuationDayResult, ValuationSnapshot, Volatility,
};
use testkit_ports::contracts::estimates::{
    EstimateLifecycleFixture, estimate_task_submission_resolution_lifecycle,
    valuation_day_append_only,
};
use testkit_ports::contracts::scenarios;

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("timestamp")
}
fn day(value: i64) -> Timestamp {
    at(value.checked_mul(86_400).expect("day"))
}
fn local(time: Timestamp) -> Timestamp {
    Timestamp::from_parts(
        time.utc_seconds(),
        time.nanosecond(),
        OffsetSeconds::new(32_400),
    )
    .expect("local")
}
fn local_midnight(value: i64) -> Timestamp {
    local(at(day(value)
        .utc_seconds()
        .as_i64()
        .checked_sub(32_400)
        .expect("midnight")))
}
fn price(value: &str) -> Price {
    Price::parse(&NumericInput::try_new(value).expect("input"), Currency::Krw).expect("price")
}
fn money(value: &str, currency: Currency) -> Money {
    Money::parse(&NumericInput::try_new(value).expect("input"), currency).expect("money")
}
fn quantity(value: &str) -> Quantity {
    Quantity::parse(&NumericInput::try_new(value).expect("input")).expect("quantity")
}
fn ratio(value: &str) -> Ratio {
    Ratio::parse(&NumericInput::try_new(value).expect("input")).expect("ratio")
}
pub(crate) fn master_for(instrument_id: &str, symbol: &str) -> MasterInstrument {
    MasterInstrument::try_new(MasterInstrumentRow {
        listing_market: MasterCode::try_new("KOSPI").expect("market"),
        instrument_id: InstrumentId::try_new(instrument_id).expect("instrument"),
        symbol: Symbol::try_new(symbol).expect("symbol"),
        name: DisplayInstrumentName::try_new("종목").expect("name"),
        security_type: MasterSecurityType::from_code(MasterCode::try_new("STOCK").expect("type")),
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
    .expect("master")
}
pub(crate) fn eligible_task(instrument_id: &str, symbol: &str, high: &str) -> CostEligibleTask {
    let source = master_for(instrument_id, symbol);
    let candles: Vec<_> = (0_i64..61)
        .map(|index| {
            let value = price(if index % 2 == 0 { "100" } else { high });
            Candle::try_new(
                source.row().instrument_id.clone(),
                CandleTime::try_new(local_midnight(index), CandlePeriod::Daily).expect("daily"),
                PriceAdjustment::Adjusted(AdjustmentBasis::new(day(61))),
                Ohlcv::try_new(value, value, value, value, quantity("10")).expect("ohlcv"),
                CandleObservation::new(day(61), CandleStatus::Finalized),
            )
            .expect("candle")
        })
        .collect();
    let volatility = Volatility::calculate(&candles).expect("volatility");
    let task = EstimateTask::create(
        EstimateContext::new(
            source
                .price_observation(price("100"), local(day(60)))
                .expect("obs"),
            day(60),
        ),
        volatility,
        Horizon::new(HorizonSelection::FewDays),
        HoursPermission::default(),
        RiskAppetite::Neutral,
    )
    .expect("task");
    let zero = CostRate::try_new(ratio("0")).expect("rate");
    let period = EstimateCostPeriod::from_calendar_result(
        &task,
        at(day(65)
            .utc_seconds()
            .as_i64()
            .checked_add(3600)
            .expect("end")),
    )
    .expect("period");
    let cost = TradeCost::from_facts(
        Some(CommissionRateFact::from_commissions_result(
            Market::Kr,
            zero,
            CommissionValidity::try_new(None, None).expect("validity"),
        )),
        Some(TradeTaxFact::from_schedule_result(Market::Kr, zero, zero)),
        period,
    )
    .expect("cost");
    CostEligibleTask::assess(task, &cost).expect("eligible")
}
fn session(day_number: i64) -> MarketSession {
    let start = local(day(day_number));
    let end = local(at(day(day_number)
        .utc_seconds()
        .as_i64()
        .checked_add(240)
        .expect("end")));
    MarketSession::try_new(
        start.offset_calendar_date(),
        SessionInformation::Kr {
            pre_market: None,
            regular_market: Some(
                SessionData::try_new(TimeInterval::try_new(start, end).expect("interval"), None)
                    .expect("session"),
            ),
            after_market: None,
        },
    )
    .expect("market session")
}
fn resolution_inputs(task: &CostEligibleTask, high: &str, low: &str) -> ResolutionInputs {
    let calendars: Vec<_> = (60_i64..=65)
        .map(|index| {
            MarketCalendar::try_new(
                session(index.checked_sub(1).expect("prev")),
                session(index),
                session(index.checked_add(1).expect("next")),
            )
            .expect("calendar")
        })
        .collect();
    let adjustment =
        OutcomeAdjustment::from_actions(task.task(), AdjustmentBasis::new(local(day(70))), &[])
            .expect("adjustment");
    let make_bar = |date: i64, minute: i64, bar_high: &str, bar_low: &str| {
        let period = if minute == 0 {
            CandlePeriod::Daily
        } else {
            CandlePeriod::Minute
        };
        let stamp = if minute == 0 {
            local_midnight(date)
        } else {
            local(at(day(date)
                .utc_seconds()
                .as_i64()
                .checked_add(minute.checked_mul(60).expect("min"))
                .expect("time")))
        };
        Candle::try_new(
            task.task().volatility().instrument().clone(),
            CandleTime::try_new(stamp, period).expect("candle time"),
            PriceAdjustment::Adjusted(adjustment.basis()),
            Ohlcv::try_new(
                price(bar_low),
                price(bar_high),
                price(bar_low),
                price(bar_low),
                quantity("10"),
            )
            .expect("ohlcv"),
            CandleObservation::new(day(70), CandleStatus::Finalized),
        )
        .expect("bar")
    };
    let mut candles: Vec<_> = (1..=4)
        .map(|minute| make_bar(60, minute, "100", "100"))
        .collect();
    candles.extend((61..=65).map(|date| make_bar(date, 0, high, low)));
    ResolutionInputs {
        calendars,
        adjustment,
        candles,
    }
}
fn outcome_for(task: &CostEligibleTask, high: &str, low: &str) -> EstimateOutcome {
    let data = resolution_inputs(task, high, low);
    EstimateOutcome::evaluate_with_complete_data(
        task.task(),
        &data.calendars,
        &data.adjustment,
        &data.candles,
        local(day(70)),
    )
}
pub(crate) fn cal_key(model_name: &str) -> CalibrationKey {
    CalibrationKey::new(
        CalibrationModel::new(
            ModelIdentityPart::try_new("claude").expect("provider"),
            ModelIdentityPart::try_new(model_name).expect("model"),
            ModelIdentityPart::try_new("v1").expect("version"),
        ),
        Horizon::new(HorizonSelection::FewDays),
    )
}
fn submission_for(task: &CostEligibleTask, key: CalibrationKey, prob: &str) -> SignalSubmission {
    SignalSubmission::create(
        SignalTaskKey::from_task(task),
        key,
        AiProbability::parse(&AiProbabilityInput::try_new(prob).expect("input")).expect("prob"),
        None,
        day(61),
    )
    .expect("submission")
}
fn snapshot_session(date: CalendarDate, market: &Market) -> MarketSession {
    let day_offset = i64::from(*date.day().as_u8())
        .checked_sub(1)
        .expect("day offset");
    let base_secs = 1_772_323_200_i64
        .checked_add(day_offset.checked_mul(86_400).expect("day"))
        .expect("secs");
    let start = Timestamp::from_parts(
        UnixSeconds::new(base_secs),
        Nanosecond::new(0),
        OffsetSeconds::new(32_400),
    )
    .expect("start");
    let end = Timestamp::from_parts(
        UnixSeconds::new(base_secs.checked_add(3600).expect("end")),
        Nanosecond::new(0),
        OffsetSeconds::new(32_400),
    )
    .expect("end");
    let regular = Some(
        SessionData::try_new(TimeInterval::try_new(start, end).expect("interval"), None)
            .expect("session"),
    );
    MarketSession::try_new(
        date,
        match market {
            Market::Kr => SessionInformation::Kr {
                pre_market: None,
                regular_market: regular,
                after_market: None,
            },
            Market::Us | Market::Unknown(_) => SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: regular,
                after_market: None,
            },
        },
    )
    .expect("market session")
}
fn march_date(day_number: u8) -> CalendarDate {
    CalendarDate::from_parts(
        CalendarYear::new(2026),
        MonthNumber::new(3),
        DayNumber::new(day_number),
    )
    .expect("date")
}
fn snapshot_on(day_number: u8, krw_cash: &str) -> ValuationSnapshot {
    let day_date = march_date(day_number);
    let prev = march_date(day_number.checked_sub(1).expect("prev"));
    let next = march_date(day_number.checked_add(1).expect("next"));
    let day_offset = i64::from(day_number).checked_sub(1).expect("offset");
    let base_secs = 1_772_323_200_i64
        .checked_add(day_offset.checked_mul(86_400).expect("day"))
        .expect("secs");
    let kr = MarketCalendar::try_new(
        snapshot_session(prev, &Market::Kr),
        snapshot_session(day_date, &Market::Kr),
        snapshot_session(next, &Market::Kr),
    )
    .expect("kr");
    let us = MarketCalendar::try_new(
        snapshot_session(prev, &Market::Us),
        snapshot_session(day_date, &Market::Us),
        snapshot_session(next, &Market::Us),
    )
    .expect("us");
    let cal = SnapshotCalendar::new(&kr, &us).expect("cal");
    let val_time = Timestamp::from_parts(
        UnixSeconds::new(base_secs.checked_add(7200).expect("val")),
        Nanosecond::new(0),
        OffsetSeconds::new(32_400),
    )
    .expect("val time");
    let portfolio = Portfolio::new(
        Vec::new(),
        BuyingPower::new(money(krw_cash, Currency::Krw), val_time),
        BuyingPower::new(money("0", Currency::Usd), val_time),
    )
    .expect("portfolio");
    ValuationSnapshot::observe(&cal, &portfolio.evaluate(None), val_time, None).expect("snapshot")
}

#[test]
fn estimate_and_valuation_sql_constants_prepare_on_migrated_database() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, timestamp()).expect("migrate");
    for statement in [
        sql::TASK_BY_ID,
        sql::INSERT_TASK,
        sql::ALL_TASKS,
        sql::CONSUME_TASK,
        sql::INSERT_SUBMISSION_RECORD,
        sql::SUBMISSION_RECORDS_BY_INSTRUMENT,
        sql::PENDING_SUBMISSION_RECORDS,
        sql::SUBMISSION_EXISTS,
        sql::INSERT_SUBMISSION,
        sql::SUBMISSIONS_BY_INSTRUMENT,
        sql::RESOLUTION_BY_TASK_KEY,
        sql::INSERT_RESOLUTION,
        sql::TASK_RESOLUTION_BY_ID,
        sql::INSERT_TASK_RESOLUTION,
        sql::INSERT_UNAVAILABLE,
        sql::CALIBRATION_HASH_EXISTS,
        sql::LATEST_CALIBRATION,
        sql::INSERT_CALIBRATION,
        sql::VALUATION_EXISTS,
        sql::INSERT_VALUATION,
        sql::VALUATION_HISTORY,
        sql::VALUATION_DAY_BY_DATE,
        sql::INSERT_VALUATION_DAY,
    ] {
        connection
            .prepare(statement)
            .expect("each runtime SQL prepares");
    }
}

fn lifecycle_fixture(
    task: &CostEligibleTask,
    out_win: &EstimateOutcome,
    out_loss: &EstimateOutcome,
) -> EstimateLifecycleFixture {
    EstimateLifecycleFixture {
        task_id: CalibrationEstimateId::try_new("life-1").expect("task id"),
        run_id: AiRunId::try_new("run-1").expect("run 1"),
        other_run_id: AiRunId::try_new("run-2").expect("run 2"),
        task: task.clone(),
        submission: submission_for(task, cal_key("lifecycle-model"), "75"),
        narrative: EstimateNarrative {
            rationale: UntrustedText::try_new("근거 문장").expect("rationale"),
            counterargument: UntrustedText::try_new("반론 문장").expect("counter"),
        },
        metadata: EstimateMetadata {
            effort: ReasoningEffort::High,
            cli_version: ModelIdentityPart::try_new("1.0.0").expect("cli"),
            prompt: EstimatePrompt {
                id: ScorecardPromptId::try_new("prompt-a").expect("prompt"),
                version: ModelIdentityPart::try_new("v1").expect("ver"),
                hash: PromptHash::try_new(&"ab".repeat(32)).expect("hash"),
            },
            evidence_bundle: EvidenceBundleId::try_new("bundle-a").expect("bundle"),
        },
        outcome: out_win.clone(),
        other_outcome: out_loss.clone(),
    }
}

#[test]
fn sqlite_estimate_store_satisfies_shared_contracts() {
    let fixture = Fixture::new();
    let database = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock())
        .expect("open database");
    let est_store = EstimateStore::new(&database);
    let task_a = eligible_task("est-sql-a", "005930", "102");
    let task_b = eligible_task("est-sql-b", "000660", "104");
    let key_a = cal_key("opus-a");
    let key_b = cal_key("opus-b");
    let sub_a = submission_for(&task_a, key_a.clone(), "72");
    let sub_b = submission_for(&task_b, key_b.clone(), "61");
    let out_win = outcome_for(&task_a, "200", "100");
    let out_loss = outcome_for(&task_a, "100", "50");
    let cal_empty = Calibration::try_new(key_a.clone(), &[]).expect("cal empty");
    let fact_a = out_win
        .calibration_fact(
            CalibrationEstimateId::try_new("cal-1").expect("id"),
            key_a.model().clone(),
            sub_a.probability(),
        )
        .expect("fact");
    let cal_one = cal_empty.with_fact(fact_a).expect("cal one");
    let cal_other = Calibration::try_new(key_b.clone(), &[]).expect("cal other");
    let lifecycle = lifecycle_fixture(&task_a, &out_win, &out_loss);
    let rt = runtime();
    rt.block_on(scenarios::estimate_append_only(
        &est_store,
        &sub_a,
        (&out_win, &out_loss),
        &sub_b,
    ));
    rt.block_on(scenarios::calibration_versions(
        &est_store,
        &key_a,
        (&cal_empty, &cal_one),
        (&key_b, &cal_other),
    ));
    rt.block_on(estimate_task_submission_resolution_lifecycle(
        &est_store, &lifecycle,
    ));
}

#[test]
fn sqlite_valuation_store_satisfies_shared_contracts() {
    let fixture = Fixture::new();
    let database = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock())
        .expect("open database");
    let val_store = ValuationStore::new(&database);
    let snap_1 = snapshot_on(10, "1000");
    let snap_2 = snapshot_on(11, "2000");
    let snap_outside = snapshot_on(15, "3000");
    let window = TimeInterval::try_new(
        snap_1.valuation_at(),
        Timestamp::from_parts(
            UnixSeconds::new(
                snap_2
                    .valuation_at()
                    .utc_seconds()
                    .as_i64()
                    .checked_add(1)
                    .expect("end"),
            ),
            Nanosecond::new(0),
            OffsetSeconds::new(32_400),
        )
        .expect("window end"),
    )
    .expect("window");
    let account_a = AccountId::new(10);
    let account_b = AccountId::new(20);
    let day_ok =
        ValuationDayResult::restore_from_saved_state(snap_1.settlement_date(), Ok(snap_1.clone()))
            .expect("day ok");
    let day_err = ValuationDayResult::restore_from_saved_state(
        snap_1.settlement_date(),
        Err(SnapshotError::MissingHistory),
    )
    .expect("day err");
    let rt = runtime();
    rt.block_on(scenarios::valuation_history(
        &val_store,
        (&account_a, &account_b),
        window,
        &[snap_1, snap_2],
        &snap_outside,
    ));
    rt.block_on(valuation_day_append_only(
        &val_store,
        (&account_a, &account_b),
        &day_ok,
        &day_err,
    ));
    assert_eq!(
        rt.block_on(val_store.day_result(&account_a, day_ok.date())),
        Ok(Some(day_ok)),
        "일별 평가 결과 조회 일치"
    );
}

struct FixedMarketData(ResolutionInputs);
impl ResolutionMarketData for FixedMarketData {
    fn inputs(
        &self,
        _record: &domain::EstimateSubmissionRecord<AiRunId, ReasoningEffort>,
        _now: Timestamp,
    ) -> impl Future<Output = Result<ResolutionInputs, ResolutionDataIssue>> + Send {
        std::future::ready(Ok(self.0.clone()))
    }
}

fn sample_submission_input(
    task_id: CalibrationEstimateId,
    task: &CostEligibleTask,
    key: &CalibrationKey,
    prob: &str,
    hash_pair: &str,
) -> application::EstimateSubmissionInput {
    application::EstimateSubmissionInput {
        task_id,
        submission: submission_for(task, key.clone(), prob),
        narrative: EstimateNarrative {
            rationale: UntrustedText::try_new("근거").expect("r"),
            counterargument: UntrustedText::try_new("반론").expect("c"),
        },
        metadata: EstimateMetadata {
            effort: ReasoningEffort::High,
            cli_version: ModelIdentityPart::try_new("1.0").expect("v"),
            prompt: EstimatePrompt {
                id: ScorecardPromptId::try_new("p").expect("p"),
                version: ModelIdentityPart::try_new("1").expect("v"),
                hash: PromptHash::try_new(&hash_pair.repeat(32)).expect("h"),
            },
            evidence_bundle: EvidenceBundleId::try_new("b-1").expect("b"),
        },
    }
}

pub(crate) fn seed_resolved_and_append_only_rows(
    rt: &tokio::runtime::Runtime,
    database: &Database,
    task_id: &CalibrationEstimateId,
) {
    let est_store = EstimateStore::new(database);
    let task = eligible_task("est-e2e-a", "005930", "102");
    let key_cal = cal_key("e2e-model");
    let run_id = AiRunId::try_new("e2e-run-1").expect("run id");
    assert_eq!(
        rt.block_on(est_store.issue_task(task_id, &run_id, &task)),
        Ok(RecordOutcome::Recorded),
        "issue task"
    );
    let bus = EventBusBuilder::default().build();
    let submit_uc = SubmitEstimate::new(&est_store, &bus);
    let input = sample_submission_input(task_id.clone(), &task, &key_cal, "74", "cd");
    assert!(
        rt.block_on(submit_uc.execute(&run_id, vec![input])).is_ok(),
        "SubmitEstimate succeeds on SQLite"
    );
    assert_eq!(
        rt.block_on(EstimateRepository::append_calibration(
            &est_store,
            &key_cal,
            &Calibration::try_new(key_cal.clone(), &[]).expect("empty")
        )),
        Ok(RecordOutcome::Recorded),
        "append empty calibration"
    );
    let market_data = FixedMarketData(resolution_inputs(&task, "200", "100"));
    let resolve_uc = ResolveEstimates::new(&est_store, &market_data, &bus);
    let reports = rt
        .block_on(resolve_uc.execute(local(day(70))))
        .expect("resolve");
    assert_eq!(reports.len(), 1, "one resolved report");
    let resolved_ok = match reports.first().map(|item| &item.status) {
        Some(ResolutionStatus::Resolved { outcome, brier, .. }) => {
            outcome.state() == OutcomeState::Success
                && brier.is_some_and(|scores| scores.before == scores.after)
        }
        Some(
            ResolutionStatus::Ongoing
            | ResolutionStatus::Unavailable(_)
            | ResolutionStatus::Storage(_)
            | ResolutionStatus::Calibration(_)
            | ResolutionStatus::MissingFact
            | ResolutionStatus::AlreadyResolved
            | ResolutionStatus::Conflict,
        )
        | None => false,
    };
    assert!(resolved_ok, "resolved with Success");
    seed_extra_append_only_rows(rt, database, &task, &key_cal);
}

fn seed_extra_append_only_rows(
    rt: &tokio::runtime::Runtime,
    database: &Database,
    task: &CostEligibleTask,
    key_cal: &CalibrationKey,
) {
    let est_store = EstimateStore::new(database);
    let val_store = ValuationStore::new(database);
    let sub = submission_for(task, key_cal.clone(), "74");
    let out = outcome_for(task, "200", "100");
    let snap = snapshot_on(12, "5000");
    let day_res =
        ValuationDayResult::restore_from_saved_state(snap.settlement_date(), Ok(snap.clone()))
            .expect("day");
    assert_eq!(
        rt.block_on(est_store.append_submission(&sub)),
        Ok(RecordOutcome::Recorded),
        "append submission"
    );
    assert_eq!(
        rt.block_on(est_store.append_resolution(sub.task_key(), &out)),
        Ok(RecordOutcome::Recorded),
        "append resolution"
    );
    assert_eq!(
        rt.block_on(application::ValuationRepository::append(
            &val_store,
            &AccountId::new(1),
            &snap
        )),
        Ok(RecordOutcome::Recorded),
        "append valuation"
    );
    assert_eq!(
        rt.block_on(val_store.record(&AccountId::new(1), &day_res)),
        Ok(RecordOutcome::Recorded),
        "record valuation day"
    );
}

#[test]
fn resolved_records_and_append_only_tables_reject_updates_and_deletes_at_sqlite_level() {
    let fixture = Fixture::new();
    let database = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock())
        .expect("open database");
    let task_id = CalibrationEstimateId::try_new("e2e-task-1").expect("task id");
    let rt = runtime();
    seed_resolved_and_append_only_rows(&rt, &database, &task_id);
    assert_triggers_reject_mutations(&rt, &database, &task_id);
}

fn assert_triggers_reject_mutations(
    rt: &tokio::runtime::Runtime,
    database: &Database,
    task_id: &CalibrationEstimateId,
) {
    for stmt in [
        test_sql::UPDATE_TASK_PAYLOAD,
        test_sql::REVERT_TASK_STATUS,
        test_sql::DELETE_TASK,
    ] {
        let id_str = task_id.as_str().to_owned();
        let err = rt
            .block_on(database.write(move |tx| tx.execute(stmt, [id_str])))
            .expect_err("task trigger must reject modification");
        assert_eq!(
            err.failure(),
            SystemFailure::RepositoryIntegrity,
            "task trigger failure"
        );
    }
    for stmt in [
        test_sql::UPDATE_SUBMISSION,
        test_sql::DELETE_SUBMISSION,
        test_sql::UPDATE_SUBMISSION_RECORD,
        test_sql::DELETE_SUBMISSION_RECORD,
        test_sql::UPDATE_RESOLUTION,
        test_sql::DELETE_RESOLUTION,
        test_sql::UPDATE_TASK_RESOLUTION,
        test_sql::DELETE_TASK_RESOLUTION,
        test_sql::UPDATE_CALIBRATION,
        test_sql::DELETE_CALIBRATION,
        test_sql::UPDATE_VALUATION,
        test_sql::DELETE_VALUATION,
        test_sql::UPDATE_VALUATION_DAY,
        test_sql::DELETE_VALUATION_DAY,
    ] {
        let err = rt
            .block_on(database.write(move |tx| tx.execute(stmt, [])))
            .expect_err("append-only trigger must reject modification");
        assert_eq!(
            err.failure(),
            SystemFailure::RepositoryIntegrity,
            "append-only trigger failure"
        );
    }
}

#[test]
fn corrupt_estimate_and_valuation_rows_fail_closed() {
    let fixture = Fixture::new();
    let database = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock())
        .expect("open database");
    let est_store = EstimateStore::new(&database);
    let val_store = ValuationStore::new(&database);
    let rt = runtime();

    rt.block_on(database.write(|tx| {
        tx.execute(test_sql::INSERT_CORRUPT_SUBMISSION, [])?;
        tx.execute(test_sql::INSERT_CORRUPT_VALUATION, [])?;
        Ok(())
    }))
    .expect("inject corrupt payloads");

    let bad_sub =
        rt.block_on(est_store.submissions(&InstrumentId::try_new("corrupt").expect("id")));
    assert_eq!(
        bad_sub
            .expect_err("corrupt submission fails closed")
            .failure(),
        SystemFailure::RepositoryIntegrity,
        "corrupt submission integrity error"
    );
    let bad_val = rt.block_on(application::ValuationRepository::history(
        &val_store,
        &AccountId::new(99),
        TimeInterval::try_new(at(0), at(2000)).expect("window"),
    ));
    assert_eq!(
        bad_val
            .expect_err("corrupt valuation fails closed")
            .failure(),
        SystemFailure::RepositoryIntegrity,
        "corrupt valuation integrity error"
    );
}

fn seed_two_pending_submissions(
    rt: &tokio::runtime::Runtime,
    est_store: &EstimateStore<'_>,
    key_cal: &CalibrationKey,
) -> (
    CostEligibleTask,
    CostEligibleTask,
    CalibrationEstimateId,
    CalibrationEstimateId,
) {
    let task_1 = eligible_task("race-a", "005930", "102");
    let task_2 = eligible_task("race-b", "000660", "104");
    let run_id = AiRunId::try_new("race-run").expect("run");
    let id_1 = CalibrationEstimateId::try_new("race-1").expect("id1");
    let id_2 = CalibrationEstimateId::try_new("race-2").expect("id2");
    rt.block_on(est_store.issue_task(&id_1, &run_id, &task_1))
        .expect("issue 1");
    rt.block_on(est_store.issue_task(&id_2, &run_id, &task_2))
        .expect("issue 2");
    let bus = EventBusBuilder::default().build();
    let submit_uc = SubmitEstimate::new(est_store, &bus);
    rt.block_on(submit_uc.execute(
        &run_id,
        vec![
            sample_submission_input(id_1.clone(), &task_1, key_cal, "70", "ef"),
            sample_submission_input(id_2.clone(), &task_2, key_cal, "70", "ef"),
        ],
    ))
    .expect("submit both");
    rt.block_on(EstimateRepository::append_calibration(
        est_store,
        key_cal,
        &Calibration::try_new(key_cal.clone(), &[]).expect("empty"),
    ))
    .expect("init cal");
    (task_1, task_2, id_1, id_2)
}

#[test]
fn stale_calibration_resolution_conflicts() {
    let fixture = Fixture::new();
    let database = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock())
        .expect("open database");
    let est_store = EstimateStore::new(&database);
    let rt = runtime();
    let key_cal = cal_key("race-model");
    let (task_1, task_2, id_1, id_2) = seed_two_pending_submissions(&rt, &est_store, &key_cal);
    let pending = rt
        .block_on(est_store.pending_submissions())
        .expect("pending");
    let rec_1 = pending.first().expect("first");
    let rec_2 = pending.get(1).expect("second");
    let stale_cal = rt
        .block_on(EstimateResolutionRepository::calibration(
            &est_store, &key_cal,
        ))
        .expect("read")
        .expect("cal");
    let out_1 = outcome_for(&task_1, "200", "100");
    let out_2 = outcome_for(&task_2, "200", "100");
    let next_1 = stale_cal
        .snapshot
        .calibration()
        .with_fact(
            out_1
                .calibration_fact(
                    id_1,
                    key_cal.model().clone(),
                    rec_1.submission().probability(),
                )
                .expect("f1"),
        )
        .expect("cal 1");
    let next_2 = stale_cal
        .snapshot
        .calibration()
        .with_fact(
            out_2
                .calibration_fact(
                    id_2,
                    key_cal.model().clone(),
                    rec_2.submission().probability(),
                )
                .expect("f2"),
        )
        .expect("cal 2");
    assert_eq!(
        rt.block_on(est_store.commit_resolution(rec_1, &out_1, &stale_cal, &next_1)),
        Ok(ResolutionCommitOutcome::Committed),
        "first resolution commits"
    );
    assert_eq!(
        rt.block_on(est_store.commit_resolution(rec_2, &out_2, &stale_cal, &next_2)),
        Ok(ResolutionCommitOutcome::Conflict),
        "stale calibration snapshot conflicts"
    );
}
