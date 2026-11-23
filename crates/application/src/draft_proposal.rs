use crate::{
    BuyLimitIntentError, Clock, ConditionalDraftOutcome, CurrentDraftOutcome, DeliveryReport,
    DraftCheck, EstimateSubmissionRepository, EvaluateSignalError, EvaluateSignalInputs, EventBus,
    ProposalRepository, SignalCommitOutcome, SignalMutation, SignalRepository, SignalSnapshot,
    StoredSignal, SystemPortError, WriteCompleted, buy_limit_intent,
};
use domain::{
    AccountId, BuyQuantityDecision, BuyQuantityFacts, BuyQuantityPlan, CheckResult,
    CostEligibleTask, CurrentBuySignal, DomainEvent, Draft, InstrumentId, IssuedEstimateTaskId,
    LimitPriceError, MarketState, MasterInstrument, MasterInstrumentError, NoBuyQuantity,
    NoSignalReason, OrderSide, Orderbook, PriceLimits, Proposal, ProposalError,
    ProposalExecutionError, ProposalId, ProposalLifetime, ProposalPriceRange, RaisedSignal,
    SignalDecision, SignalTiming, SignalTransitionError, SignalUnavailable, TickSizeError,
    TickTable, Timestamp, calculate_buy_quantity, calculate_limit_price, signal_transition,
};
use std::future::Future;

#[derive(Debug, Clone)]
pub struct DraftProposalInputs {
    pub evaluation: EvaluateSignalInputs,
    pub master: MasterInstrument,
    pub book: Orderbook,
    pub limits: PriceLimits,
}

pub trait DraftProposalData: Send + Sync {
    fn inputs(
        &self,
        instrument: &InstrumentId,
        account: &AccountId,
        current: &StoredSignal,
        now: Timestamp,
    ) -> impl Future<Output = Result<DraftProposalInputs, SystemPortError>> + Send;
}

pub trait ReestimateScheduler: Send + Sync {
    fn schedule(
        &self,
        instrument: &InstrumentId,
        task: &IssuedEstimateTaskId,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send;
}

#[derive(Debug)]
pub enum DraftProposalOutcome {
    Created {
        proposal: Box<Proposal<Draft>>,
        delivery: DeliveryReport,
    },
    Existing {
        proposal_id: ProposalId,
    },
    NoSignal,
    MarketClosed,
    ReestimateScheduled {
        deliveries: Vec<DeliveryReport>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DraftProposalError {
    Storage(SystemPortError),
    Inputs(SystemPortError),
    Evaluation(EvaluateSignalError),
    Master(MasterInstrumentError),
    InstrumentMismatch,
    MarketUnknown,
    Observation(SignalUnavailable),
    SignalUnavailable(NoSignalReason),
    SignalChanged,
    MissingQuantityFacts,
    Quantity(NoBuyQuantity),
    Tick(TickSizeError),
    Limit(LimitPriceError),
    BuyIntent(BuyLimitIntentError),
    Proposal(ProposalError),
    DraftStatus(ProposalExecutionError),
    Transition(SignalTransitionError),
    SubmissionDiscarded,
    SellDraftUnavailable,
    Schedule {
        error: SystemPortError,
        deliveries: Vec<DeliveryReport>,
    },
    ConcurrentUpdate,
}

struct DraftRequest<'a> {
    instrument: &'a InstrumentId,
    account: &'a AccountId,
    id: &'a ProposalId,
    now: Timestamp,
}

struct PreparedDraft {
    inputs: DraftProposalInputs,
    range: ProposalPriceRange,
}

enum Preparation {
    Closed,
    Ready(Box<PreparedDraft>),
}

enum Attempt {
    Complete(DraftProposalOutcome),
    Retry(DraftProposalError),
}

#[derive(Debug)]
pub struct DraftProposal<'a, Repository, Data, Scheduler, Time> {
    repository: &'a Repository,
    data: &'a Data,
    scheduler: &'a Scheduler,
    clock: &'a Time,
    events: &'a EventBus,
}

impl<'a, Repository, Data, Scheduler, Time> DraftProposal<'a, Repository, Data, Scheduler, Time>
where
    Repository: SignalRepository + EstimateSubmissionRepository + ProposalRepository,
    Data: DraftProposalData,
    Scheduler: ReestimateScheduler,
    Time: Clock,
{
    #[must_use]
    pub const fn new(
        repository: &'a Repository,
        data: &'a Data,
        scheduler: &'a Scheduler,
        clock: &'a Time,
        events: &'a EventBus,
    ) -> Self {
        Self {
            repository,
            data,
            scheduler,
            clock,
            events,
        }
    }

    pub async fn execute(
        &self,
        instrument: &InstrumentId,
        account: &AccountId,
        id: ProposalId,
    ) -> Result<DraftProposalOutcome, DraftProposalError> {
        let mut last_retry = DraftProposalError::ConcurrentUpdate;
        for _ in 0_u8..3 {
            let request = DraftRequest {
                instrument,
                account,
                id: &id,
                now: self.clock.now(),
            };
            match self.attempt(&request).await? {
                Attempt::Complete(result) => return Ok(result),
                Attempt::Retry(reason) => last_retry = reason,
            }
        }
        Err(last_retry)
    }

    async fn attempt(&self, request: &DraftRequest<'_>) -> Result<Attempt, DraftProposalError> {
        let snapshot = self
            .repository
            .snapshot(request.instrument)
            .await
            .map_err(DraftProposalError::Storage)?;
        let Some(current) = snapshot.current.as_ref() else {
            return Ok(Attempt::Complete(DraftProposalOutcome::NoSignal));
        };
        if snapshot.submission.as_ref() != Some(&current.submission) {
            return Ok(Attempt::Retry(DraftProposalError::SignalChanged));
        }
        let Some(preparation) = self.prepare(request, current).await? else {
            return Ok(Attempt::Retry(DraftProposalError::SignalChanged));
        };
        let Preparation::Ready(prepared) = preparation else {
            return Ok(Attempt::Complete(DraftProposalOutcome::MarketClosed));
        };
        self.finish_attempt(request, &snapshot, current, &prepared)
            .await
    }

    async fn finish_attempt(
        &self,
        request: &DraftRequest<'_>,
        snapshot: &SignalSnapshot,
        current: &StoredSignal,
        prepared: &PreparedDraft,
    ) -> Result<Attempt, DraftProposalError> {
        let Some(read_at) = self.recheck(current, &prepared.inputs)? else {
            return Ok(Attempt::Complete(DraftProposalOutcome::MarketClosed));
        };
        let observation = prepared.inputs.evaluation.target_price.as_ref().ok_or(
            DraftProposalError::Observation(SignalUnavailable::MissingTargetPrice),
        )?;
        if prepared.range.check(observation.price()) != CheckResult::Passed {
            return self.discard(request.instrument, snapshot, current).await;
        }
        if let RaisedSignal::Sell(_) = &current.signal {
            return Err(DraftProposalError::SellDraftUnavailable);
        }
        let read_check = DraftCheck {
            at: read_at,
            observation,
        };
        match self
            .repository
            .current_draft(request.instrument, snapshot, read_check)
            .await
            .map_err(DraftProposalError::Storage)?
        {
            CurrentDraftOutcome::Absent => {}
            CurrentDraftOutcome::Existing(proposal_id) => {
                return Ok(Attempt::Complete(DraftProposalOutcome::Existing {
                    proposal_id,
                }));
            }
            CurrentDraftOutcome::Conflict => {
                return Ok(Attempt::Retry(DraftProposalError::ConcurrentUpdate));
            }
            CurrentDraftOutcome::Invalid(error) => {
                return Err(DraftProposalError::DraftStatus(error));
            }
        }
        let proposal = self.make_proposal(request, current, prepared).await?;
        let Some(write_at) = self.recheck(current, &prepared.inputs)? else {
            return Ok(Attempt::Complete(DraftProposalOutcome::MarketClosed));
        };
        proposal
            .check_current(write_at, observation)
            .map_err(DraftProposalError::DraftStatus)?;
        let write_check = DraftCheck {
            at: write_at,
            observation,
        };
        self.store_candidate(request.instrument, snapshot, proposal, write_check)
            .await
    }

    async fn store_candidate(
        &self,
        instrument: &InstrumentId,
        snapshot: &SignalSnapshot,
        proposal: Proposal<Draft>,
        check: DraftCheck<'_>,
    ) -> Result<Attempt, DraftProposalError> {
        match self
            .repository
            .store_current_draft(instrument, snapshot, &proposal, check)
            .await
            .map_err(DraftProposalError::Storage)?
        {
            ConditionalDraftOutcome::Conflict => {
                Ok(Attempt::Retry(DraftProposalError::ConcurrentUpdate))
            }
            ConditionalDraftOutcome::Existing(proposal_id) => {
                Ok(Attempt::Complete(DraftProposalOutcome::Existing {
                    proposal_id,
                }))
            }
            ConditionalDraftOutcome::Invalid(ProposalExecutionError::ClockRollback) => {
                Ok(Attempt::Retry(DraftProposalError::DraftStatus(
                    ProposalExecutionError::ClockRollback,
                )))
            }
            ConditionalDraftOutcome::Invalid(
                error @ (ProposalExecutionError::Expired
                | ProposalExecutionError::PriceOutsideRange
                | ProposalExecutionError::ObservationInvalid),
            ) => Err(DraftProposalError::DraftStatus(error)),
            ConditionalDraftOutcome::Stored => {
                let delivery = self
                    .events
                    .publish(DomainEvent::ProposalCreated(proposal.id().clone()));
                Ok(Attempt::Complete(DraftProposalOutcome::Created {
                    proposal: Box::new(proposal),
                    delivery,
                }))
            }
        }
    }

    async fn prepare(
        &self,
        request: &DraftRequest<'_>,
        current: &StoredSignal,
    ) -> Result<Option<Preparation>, DraftProposalError> {
        let records = self
            .repository
            .submission_records(request.instrument)
            .await
            .map_err(DraftProposalError::Storage)?;
        let Some(record) = records.last() else {
            return Ok(None);
        };
        if record.task_id() != &current.submission {
            return Ok(None);
        }
        let assignments = self
            .repository
            .assignments()
            .await
            .map_err(DraftProposalError::Storage)?;
        let issued = assignments
            .lookup(record.task_id().as_estimate_id())
            .map_err(|error| {
                DraftProposalError::Evaluation(EvaluateSignalError::Assignment(error))
            })?;
        let inputs = self
            .data
            .inputs(request.instrument, request.account, current, request.now)
            .await
            .map_err(DraftProposalError::Inputs)?;
        match market_state(
            &inputs.evaluation,
            &signal_task(current).reference().instrument().market(),
            request.now,
        ) {
            MarketState::Unknown => return Err(DraftProposalError::MarketUnknown),
            MarketState::Closed => return Ok(Some(Preparation::Closed)),
            MarketState::DayMarket
            | MarketState::PreMarket
            | MarketState::RegularMarket
            | MarketState::AfterMarket => {}
        }
        check_identity(request.instrument, current, issued.task(), &inputs.master)?;
        self.prepare_open(request, current, (record, issued.task()), inputs)
            .await
            .map(Some)
    }

    async fn prepare_open(
        &self,
        request: &DraftRequest<'_>,
        current: &StoredSignal,
        assessment: (
            &domain::EstimateSubmissionRecord<crate::AiRunId, crate::ReasoningEffort>,
            &CostEligibleTask,
        ),
        inputs: DraftProposalInputs,
    ) -> Result<Preparation, DraftProposalError> {
        let (record, issued) = assessment;
        let observation =
            inputs
                .evaluation
                .target_price
                .as_ref()
                .ok_or(DraftProposalError::Observation(
                    SignalUnavailable::MissingTargetPrice,
                ))?;
        observation
            .validate_for_task(signal_task(current), request.now)
            .map_err(DraftProposalError::Observation)?;
        let sources = self
            .repository
            .sources(record.task_id())
            .await
            .map_err(DraftProposalError::Storage)?;
        let reassessed = crate::evaluate_signals::assess(
            record,
            issued,
            &inputs.evaluation,
            &sources,
            request.now,
        )
        .map_err(DraftProposalError::Evaluation)?;
        match &reassessed {
            SignalDecision::None(reason) => {
                return Err(DraftProposalError::SignalUnavailable(*reason));
            }
            SignalDecision::Buy(_) | SignalDecision::Sell(_) => {}
        }
        if reassessed.into_signal().as_ref() != Some(&current.signal) {
            return Err(DraftProposalError::SignalChanged);
        }
        let range = ProposalPriceRange::from_estimate_task(
            signal_task(current).range_facts(),
            observation.clone(),
        )
        .map_err(DraftProposalError::Proposal)?;
        Ok(Preparation::Ready(Box::new(PreparedDraft {
            inputs,
            range,
        })))
    }

    async fn make_proposal(
        &self,
        request: &DraftRequest<'_>,
        current: &StoredSignal,
        prepared: &PreparedDraft,
    ) -> Result<Proposal<Draft>, DraftProposalError> {
        let RaisedSignal::Buy(buy) = &current.signal else {
            return Err(DraftProposalError::SellDraftUnavailable);
        };
        let occurrence = current
            .occurrence
            .as_ref()
            .ok_or(DraftProposalError::Quantity(NoBuyQuantity::Indeterminate))?;
        let current_buy = CurrentBuySignal::try_new(buy.clone(), occurrence.clone())
            .map_err(DraftProposalError::Quantity)?;
        let (plan, calculated_at) = self
            .buy_plan(request, &current_buy, &prepared.inputs.evaluation)
            .await?;
        let table =
            TickTable::from_master(&prepared.inputs.master).map_err(DraftProposalError::Tick)?;
        let limit = calculate_limit_price(
            &prepared.inputs.book,
            Some(&table),
            &prepared.inputs.limits,
            OrderSide::Buy,
            calculated_at,
        )
        .map_err(DraftProposalError::Limit)?;
        let intent = buy_limit_intent(
            &plan,
            &limit,
            request.account.clone(),
            request.id.as_client_order_id().clone(),
        )
        .map_err(DraftProposalError::BuyIntent)?;
        Proposal::new(
            request.id.clone(),
            intent,
            calculated_at,
            prepared.range.clone(),
            ProposalLifetime::default(),
        )
        .map_err(DraftProposalError::Proposal)
    }

    async fn buy_plan(
        &self,
        request: &DraftRequest<'_>,
        current: &CurrentBuySignal,
        inputs: &EvaluateSignalInputs,
    ) -> Result<(BuyQuantityPlan, Timestamp), DraftProposalError> {
        let history = self
            .repository
            .occurrences(
                request.instrument,
                current.signal().facts().task().reference().instrument(),
            )
            .await
            .map_err(DraftProposalError::Storage)?;
        let valuation = inputs
            .valuation
            .as_ref()
            .ok_or(DraftProposalError::MissingQuantityFacts)?;
        let costs = inputs
            .quantity_costs
            .as_ref()
            .ok_or(DraftProposalError::MissingQuantityFacts)?;
        let observation = inputs
            .target_price
            .as_ref()
            .ok_or(DraftProposalError::Observation(
                SignalUnavailable::MissingTargetPrice,
            ))?;
        let facts = BuyQuantityFacts::new(
            valuation,
            &inputs.policy,
            (&inputs.assignments, &inputs.security),
            observation,
            (costs.fx, &costs.commission),
        );
        let calculated_at = self.clock.now();
        match calculate_buy_quantity(current, &facts, calculated_at, &inputs.calendars, &history) {
            BuyQuantityDecision::Plan(plan) => Ok((*plan, calculated_at)),
            BuyQuantityDecision::None(reason) => Err(DraftProposalError::Quantity(reason)),
        }
    }

    fn recheck(
        &self,
        current: &StoredSignal,
        inputs: &DraftProposalInputs,
    ) -> Result<Option<Timestamp>, DraftProposalError> {
        let now = self.clock.now();
        let task = signal_task(current);
        let state = market_state(
            &inputs.evaluation,
            &task.reference().instrument().market(),
            now,
        );
        match state {
            MarketState::Unknown => return Err(DraftProposalError::MarketUnknown),
            MarketState::Closed => return Ok(None),
            MarketState::DayMarket
            | MarketState::PreMarket
            | MarketState::RegularMarket
            | MarketState::AfterMarket => {}
        }
        let observation =
            inputs
                .evaluation
                .target_price
                .as_ref()
                .ok_or(DraftProposalError::Observation(
                    SignalUnavailable::MissingTargetPrice,
                ))?;
        observation
            .validate_for_task(task, now)
            .map_err(DraftProposalError::Observation)?;
        let submission = match &current.signal {
            RaisedSignal::Buy(value) => value.facts().submission(),
            RaisedSignal::Sell(value) => value.facts().submission(),
        };
        SignalTiming::new(now, &inputs.evaluation.calendars)
            .check(task, submission.submitted_at())
            .map_err(DraftProposalError::SignalUnavailable)?;
        Ok(Some(now))
    }

    async fn discard(
        &self,
        instrument: &InstrumentId,
        snapshot: &SignalSnapshot,
        current: &StoredSignal,
    ) -> Result<Attempt, DraftProposalError> {
        let transition = signal_transition(
            Some(&current.signal),
            &SignalDecision::None(NoSignalReason::Expired),
        )
        .map_err(DraftProposalError::Transition)?;
        let mutation = SignalMutation::Discard(Box::new(transition));
        match self
            .repository
            .commit(instrument, snapshot, &mutation)
            .await
            .map_err(DraftProposalError::Storage)?
        {
            SignalCommitOutcome::Conflict => {
                Ok(Attempt::Retry(DraftProposalError::ConcurrentUpdate))
            }
            SignalCommitOutcome::SubmissionDiscarded => {
                Err(DraftProposalError::SubmissionDiscarded)
            }
            SignalCommitOutcome::Applied(commit) => {
                let deliveries: Vec<_> = commit
                    .events
                    .into_iter()
                    .map(|event| self.events.publish(event))
                    .collect();
                self.scheduler
                    .schedule(instrument, &current.submission)
                    .await
                    .map_err(|error| DraftProposalError::Schedule {
                        error,
                        deliveries: deliveries.clone(),
                    })?;
                Ok(Attempt::Complete(
                    DraftProposalOutcome::ReestimateScheduled { deliveries },
                ))
            }
        }
    }
}

fn signal_task(current: &StoredSignal) -> &domain::EstimateTask {
    match &current.signal {
        RaisedSignal::Buy(value) => value.facts().task(),
        RaisedSignal::Sell(value) => value.facts().task(),
    }
}

fn check_identity(
    instrument: &InstrumentId,
    current: &StoredSignal,
    issued: &CostEligibleTask,
    source: &MasterInstrument,
) -> Result<(), DraftProposalError> {
    let side = match &current.signal {
        RaisedSignal::Buy(_) => OrderSide::Buy,
        RaisedSignal::Sell(_) => OrderSide::Sell,
    };
    let master = source
        .order_gate_facts()
        .and_then(|facts| facts.instrument_for(side))
        .map_err(DraftProposalError::Master)?;
    let task = signal_task(current);
    if source.row().instrument_id != *instrument
        || task.volatility().instrument() != instrument
        || task.reference().instrument() != &master
        || issued.task() != task
    {
        return Err(DraftProposalError::InstrumentMismatch);
    }
    Ok(())
}

fn market_state(
    inputs: &EvaluateSignalInputs,
    market: &domain::Market,
    now: Timestamp,
) -> MarketState {
    let mut known = None;
    for calendar in &inputs.calendars {
        if &calendar.today().market() != market {
            continue;
        }
        let state = calendar.state_at(now);
        if state == MarketState::Unknown {
            continue;
        }
        match known {
            Some(previous) if previous != state => return MarketState::Unknown,
            Some(_) | None => known = Some(state),
        }
    }
    known.unwrap_or(MarketState::Unknown)
}
