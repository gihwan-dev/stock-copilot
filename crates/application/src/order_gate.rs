use crate::{
    AuditEntry, AuditKind, AuditLog, BrokerRead, Clock, MarketInfo, PortError, SystemPortError,
    ValidationReportRepository,
};
use domain::{
    AccountDisplayFact, AccountId, BrokerAccountKeySelection, BrokerAccountRows, BrokerOrderSet,
    BuyingPower, CommissionRateFact, Currency, Draft, DraftOrderIntent, GateCheckDecision,
    InstrumentNameFact, KnownMarket, Market, MarketCalendar, MarketOrderPolicy, MasterInstrument,
    MasterInstrumentError, OrderFxRates, OrderGateFacts, OrderIntent, OrderSide,
    PolicySecurityFacts, PortfolioValuation, PriceLimits, PriceObservation, Proposal, ProposalId,
    QuoteInstrument, ResolvedOrderInstrument, SectorAssignments, SellableQuantityFact,
    StockWarnings, Timestamp, Validated, ValidationReport, recheck_intent_gate,
    select_brokerage_account_by_id,
};

#[derive(Debug, Clone)]
pub struct OrderGatePolicyContext {
    pub master: MasterInstrument,
    pub market_order_policy: MarketOrderPolicy,
    pub valuation: PortfolioValuation,
    pub assignments: SectorAssignments,
    pub security_facts: PolicySecurityFacts,
}

#[derive(Debug, Clone)]
pub struct OrderGateCollectedFacts {
    pub account_display: AccountDisplayFact,
    pub instrument_name: InstrumentNameFact,
    pub commission_rate: Option<CommissionRateFact>,
    pub order_fx_rates: OrderFxRates,
    pub validated_at: Timestamp,
    pub observation: PriceObservation,
}

#[derive(Debug)]
pub enum OrderGateOutcome {
    Passed {
        validated: Box<Proposal<Validated>>,
        report: ValidationReport,
        collected: Box<OrderGateCollectedFacts>,
    },
    Rejected {
        report: ValidationReport,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentRecheckOutcome {
    Passed {
        checked_at: Timestamp,
        observation: PriceObservation,
    },
    Rejected {
        report: ValidationReport,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderGateError {
    Broker(PortError),
    Storage(SystemPortError),
    Master(MasterInstrumentError),
    UnknownMarket,
    MissingQuote,
    InvalidObservation,
}

struct LiveDataTarget<'a> {
    account_id: &'a AccountId,
    instrument: &'a ResolvedOrderInstrument,
    side: OrderSide,
    market: Market,
}

struct LiveOrderGateData {
    accounts: BrokerAccountRows,
    calendar: MarketCalendar,
    observation: PriceObservation,
    price_limits: PriceLimits,
    open_orders: Option<BrokerOrderSet>,
    buying_power: Option<BuyingPower>,
    sellable_quantity: Option<SellableQuantityFact>,
    stock_warnings: Option<StockWarnings>,
    commission_rate: Option<CommissionRateFact>,
    order_fx_rates: OrderFxRates,
}

struct SideFacts {
    buying_power: Option<BuyingPower>,
    sellable_quantity: Option<SellableQuantityFact>,
    stock_warnings: Option<StockWarnings>,
    commission_rate: Option<CommissionRateFact>,
    order_fx_rates: OrderFxRates,
}

#[derive(Debug)]
pub struct ValidateOrderProposal<'a, Broker, MarketSource, Reports, Audit, Time> {
    broker: &'a Broker,
    market: &'a MarketSource,
    reports: &'a Reports,
    audit: &'a Audit,
    clock: &'a Time,
}

impl<'a, Broker, MarketSource, Reports, Audit, Time>
    ValidateOrderProposal<'a, Broker, MarketSource, Reports, Audit, Time>
where
    Broker: BrokerRead,
    MarketSource: MarketInfo,
    Reports: ValidationReportRepository,
    Audit: AuditLog,
    Time: Clock,
{
    #[must_use]
    pub const fn new(
        broker: &'a Broker,
        market: &'a MarketSource,
        reports: &'a Reports,
        audit: &'a Audit,
        clock: &'a Time,
    ) -> Self {
        Self {
            broker,
            market,
            reports,
            audit,
            clock,
        }
    }

    pub async fn execute(
        &self,
        proposal: Proposal<Draft>,
        context: &OrderGatePolicyContext,
    ) -> Result<OrderGateOutcome, OrderGateError> {
        let draft_intent = proposal.intent();
        let proposal_id = proposal.id().clone();
        let target = LiveDataTarget {
            account_id: draft_intent.account_id(),
            instrument: draft_intent.instrument(),
            side: draft_intent.side(),
            market: draft_intent.market(),
        };
        let live = self.collect_live_data(&target, &context.master).await?;
        let now = self.clock.now();
        let calendars = [live.calendar];
        let facts = OrderGateFacts {
            master: &context.master,
            accounts: &live.accounts,
            calendars: &calendars,
            market_order_policy: &context.market_order_policy,
            valuation: &context.valuation,
            assignments: &context.assignments,
            security_facts: &context.security_facts,
            buying_power: live.buying_power,
            sellable_quantity: live.sellable_quantity.as_ref(),
            open_orders: live.open_orders.as_ref(),
            stock_warnings: live.stock_warnings.as_ref(),
            price_limits: Some(&live.price_limits),
            commission_rate: live.commission_rate.as_ref(),
            order_fx_rates: live.order_fx_rates,
        };
        let report = proposal.evaluate_gate(now, &live.observation, &facts);
        let outcome = proposal.validate_with_facts(now, live.observation.clone(), &facts);
        self.record_validation(&proposal_id, &report, now).await?;
        build_outcome(
            outcome,
            report,
            FinalizeMeta {
                draft_intent: &draft_intent,
                context,
                accounts: live.accounts,
                commission_rate: live.commission_rate,
                order_fx_rates: live.order_fx_rates,
                validated_at: now,
                observation: live.observation,
            },
        )
    }

    pub async fn recheck_intent(
        &self,
        intent: &OrderIntent,
        context: &OrderGatePolicyContext,
    ) -> Result<IntentRecheckOutcome, OrderGateError> {
        let target = LiveDataTarget {
            account_id: intent.account_id(),
            instrument: intent.instrument(),
            side: intent.side(),
            market: intent.market(),
        };
        let live = self.collect_live_data(&target, &context.master).await?;
        let now = self.clock.now();
        let calendars = [live.calendar];
        let facts = OrderGateFacts {
            master: &context.master,
            accounts: &live.accounts,
            calendars: &calendars,
            market_order_policy: &context.market_order_policy,
            valuation: &context.valuation,
            assignments: &context.assignments,
            security_facts: &context.security_facts,
            buying_power: live.buying_power,
            sellable_quantity: live.sellable_quantity.as_ref(),
            open_orders: live.open_orders.as_ref(),
            stock_warnings: live.stock_warnings.as_ref(),
            price_limits: Some(&live.price_limits),
            commission_rate: live.commission_rate.as_ref(),
            order_fx_rates: live.order_fx_rates,
        };
        let (decision, report) = recheck_intent_gate(intent, now, &live.observation, &facts);
        match decision {
            GateCheckDecision::Passed => Ok(IntentRecheckOutcome::Passed {
                checked_at: now,
                observation: live.observation,
            }),
            GateCheckDecision::Rejected => Ok(IntentRecheckOutcome::Rejected { report }),
        }
    }

    async fn collect_live_data(
        &self,
        target: &LiveDataTarget<'_>,
        master: &MasterInstrument,
    ) -> Result<LiveOrderGateData, OrderGateError> {
        let known_market = KnownMarket::try_from_market(&target.market)
            .map_err(|_error| OrderGateError::UnknownMarket)?;
        let quote_instrument = master.quote_instrument().map_err(OrderGateError::Master)?;
        let accounts = self
            .broker
            .accounts()
            .await
            .map_err(OrderGateError::Broker)?;
        let calendar = self
            .market
            .calendar(known_market)
            .await
            .map_err(OrderGateError::Broker)?;
        let observation = self
            .fetch_observation(target.side, master, &quote_instrument)
            .await?;
        let price_limits = self
            .broker
            .price_limits(&quote_instrument)
            .await
            .map_err(OrderGateError::Broker)?;
        let open_orders = match known_market {
            KnownMarket::Kr => Some(
                self.broker
                    .open_orders(target.account_id)
                    .await
                    .map_err(OrderGateError::Broker)?,
            ),
            KnownMarket::Us => None,
        };
        let side = self.fetch_side_facts(target, known_market).await?;
        Ok(LiveOrderGateData {
            accounts,
            calendar,
            observation,
            price_limits,
            open_orders,
            buying_power: side.buying_power,
            sellable_quantity: side.sellable_quantity,
            stock_warnings: side.stock_warnings,
            commission_rate: side.commission_rate,
            order_fx_rates: side.order_fx_rates,
        })
    }

    async fn record_validation(
        &self,
        proposal_id: &ProposalId,
        report: &ValidationReport,
        now: Timestamp,
    ) -> Result<(), OrderGateError> {
        let _written = self
            .reports
            .append(proposal_id, std::slice::from_ref(report))
            .await
            .map_err(OrderGateError::Storage)?;
        let _audit_outcome = self
            .audit
            .append(&AuditEntry::new(
                now,
                AuditKind::ValidationRecorded(proposal_id.clone()),
            ))
            .await
            .map_err(OrderGateError::Storage)?;
        Ok(())
    }

    async fn fetch_observation(
        &self,
        side: OrderSide,
        master: &MasterInstrument,
        quote_instrument: &QuoteInstrument,
    ) -> Result<PriceObservation, OrderGateError> {
        let quotes = self
            .broker
            .quotes(std::slice::from_ref(quote_instrument))
            .await
            .map_err(OrderGateError::Broker)?;
        let matched = quotes
            .into_iter()
            .find(|entry| &entry.instrument == quote_instrument)
            .ok_or(OrderGateError::MissingQuote)?;
        let quote = matched.result.map_err(OrderGateError::Broker)?;
        let price = quote.last_price().ok_or(OrderGateError::MissingQuote)?;
        let observed_at = quote.timestamp().unwrap_or_else(|| quote.collected_at());
        master
            .price_observation_for(side, price, observed_at)
            .map_err(|_error| OrderGateError::InvalidObservation)
    }

    async fn fetch_side_facts(
        &self,
        target: &LiveDataTarget<'_>,
        known_market: KnownMarket,
    ) -> Result<SideFacts, OrderGateError> {
        let commission_rate = self
            .broker
            .commission_rate(target.account_id, known_market)
            .await
            .ok();
        let order_fx_rates = match known_market {
            KnownMarket::Us => match self.market.order_fx_rates().await {
                Ok(rates) => rates,
                Err(_) => OrderFxRates::missing(),
            },
            KnownMarket::Kr => OrderFxRates::missing(),
        };
        match target.side {
            OrderSide::Buy => {
                let currency = match known_market {
                    KnownMarket::Kr => Currency::Krw,
                    KnownMarket::Us => Currency::Usd,
                };
                let buying_power = self
                    .broker
                    .buying_power(target.account_id, currency)
                    .await
                    .map_err(OrderGateError::Broker)?;
                let warnings = self
                    .broker
                    .stock_warnings(target.instrument)
                    .await
                    .map_err(OrderGateError::Broker)?;
                Ok(SideFacts {
                    buying_power: Some(buying_power),
                    sellable_quantity: None,
                    stock_warnings: Some(warnings),
                    commission_rate,
                    order_fx_rates,
                })
            }
            OrderSide::Sell => {
                let sellable = self
                    .broker
                    .sellable_quantity(target.account_id, target.instrument)
                    .await
                    .map_err(OrderGateError::Broker)?;
                Ok(SideFacts {
                    buying_power: None,
                    sellable_quantity: Some(sellable),
                    stock_warnings: None,
                    commission_rate,
                    order_fx_rates,
                })
            }
        }
    }
}

struct FinalizeMeta<'a> {
    draft_intent: &'a DraftOrderIntent,
    context: &'a OrderGatePolicyContext,
    accounts: BrokerAccountRows,
    commission_rate: Option<CommissionRateFact>,
    order_fx_rates: OrderFxRates,
    validated_at: Timestamp,
    observation: PriceObservation,
}

fn build_outcome(
    outcome: Result<Proposal<Validated>, ValidationReport>,
    report: ValidationReport,
    meta: FinalizeMeta<'_>,
) -> Result<OrderGateOutcome, OrderGateError> {
    match outcome {
        Ok(validated) => {
            let account_display = match select_brokerage_account_by_id(
                &meta.accounts,
                meta.draft_intent.account_id(),
            ) {
                BrokerAccountKeySelection::Selected(row) => row.display_fact(),
                BrokerAccountKeySelection::Empty
                | BrokerAccountKeySelection::NotFound
                | BrokerAccountKeySelection::NotBrokerage => {
                    return Ok(OrderGateOutcome::Rejected { report });
                }
            };
            let instrument_name = meta
                .context
                .master
                .name_fact_for(meta.draft_intent.side())
                .map_err(OrderGateError::Master)?;
            Ok(OrderGateOutcome::Passed {
                validated: Box::new(validated),
                report,
                collected: Box::new(OrderGateCollectedFacts {
                    account_display,
                    instrument_name,
                    commission_rate: meta.commission_rate,
                    order_fx_rates: meta.order_fx_rates,
                    validated_at: meta.validated_at,
                    observation: meta.observation,
                }),
            })
        }
        Err(failed_report) => Ok(OrderGateOutcome::Rejected {
            report: failed_report,
        }),
    }
}
