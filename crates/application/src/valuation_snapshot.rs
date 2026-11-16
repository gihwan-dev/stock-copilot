use crate::{
    Clock, DeliveryReport, EventBus, FxRateQuery, MarketDataRepository, PortfolioRepository,
    RecordOutcome, SystemPortError, ValuationRepository,
};
use domain::{
    AccountId, CalendarDate, Currency, DomainEvent, FxPair, FxRateKind, SnapshotCalendar,
    SnapshotError, SnapshotPriceBasis, SyncOutcome, Timestamp, ValuationSnapshot,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValuationSnapshotError {
    Repository(SystemPortError),
    MissingPortfolio,
    UnavailablePortfolio,
    PriceBasisDefinitionPending,
    Snapshot(SnapshotError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValuationSnapshotReport {
    pub snapshot: ValuationSnapshot,
    pub recorded: RecordOutcome,
    pub delivery: DeliveryReport,
}

#[derive(Debug)]
pub struct TakeValuationSnapshot<'a, Portfolio, Market, Repository, Time> {
    portfolio: &'a Portfolio,
    market: &'a Market,
    repository: &'a Repository,
    clock: &'a Time,
    events: &'a EventBus,
}
impl<'a, Portfolio, Market, Repository, Time>
    TakeValuationSnapshot<'a, Portfolio, Market, Repository, Time>
where
    Portfolio: PortfolioRepository,
    Market: MarketDataRepository,
    Repository: ValuationRepository,
    Time: Clock,
{
    #[must_use]
    pub const fn new(
        portfolio: &'a Portfolio,
        market: &'a Market,
        repository: &'a Repository,
        clock: &'a Time,
        events: &'a EventBus,
    ) -> Self {
        Self {
            portfolio,
            market,
            repository,
            clock,
            events,
        }
    }
    pub async fn execute(
        &self,
        account: &AccountId,
        calendar: &SnapshotCalendar,
    ) -> Result<ValuationSnapshotReport, ValuationSnapshotError> {
        let record = self
            .portfolio
            .latest(account)
            .await
            .map_err(ValuationSnapshotError::Repository)?
            .ok_or(ValuationSnapshotError::MissingPortfolio)?;
        let portfolio = match record.outcome() {
            SyncOutcome::Complete(value) | SyncOutcome::Partial(value) => value,
            SyncOutcome::Failed | SyncOutcome::AccessBlocked => {
                return Err(ValuationSnapshotError::UnavailablePortfolio);
            }
        };
        let at = validate_observation_at(record.recorded_at(), self.clock.now())?;
        let fx = self
            .market
            .fx_rate(FxPair::UsdKrw, FxRateKind::Mid, FxRateQuery::At(at))
            .await
            .map_err(ValuationSnapshotError::Repository)?;
        let snapshot = ValuationSnapshot::observe(calendar, &portfolio.evaluate(fx), at, fx)
            .map_err(ValuationSnapshotError::Snapshot)?;
        validate_price_basis(
            snapshot.price_basis(Currency::Krw),
            price_requirement(portfolio, Currency::Krw),
        )?;
        validate_price_basis(
            snapshot.price_basis(Currency::Usd),
            price_requirement(portfolio, Currency::Usd),
        )?;
        validate_portfolio_observations(portfolio, calendar, &snapshot, at)?;
        let recorded = self
            .repository
            .append(account, &snapshot)
            .await
            .map_err(ValuationSnapshotError::Repository)?;
        let delivery = self
            .events
            .publish(DomainEvent::ValuationSnapshotTaken(account.clone()));
        Ok(ValuationSnapshotReport {
            snapshot,
            recorded,
            delivery,
        })
    }
}

fn validate_portfolio_observations(
    portfolio: &domain::Portfolio,
    calendar: &SnapshotCalendar,
    snapshot: &ValuationSnapshot,
    at: Timestamp,
) -> Result<(), ValuationSnapshotError> {
    for holding in portfolio.holdings() {
        validate_observation_at(holding.observed_at(), at)?;
        let price_date = calendar
            .price_date(holding.currency(), holding.observed_at())
            .map_err(ValuationSnapshotError::Snapshot)?;
        validate_price_date(price_date, snapshot.price_date(holding.currency()))?;
    }
    if let Some(reported) = portfolio.reported() {
        validate_observation_at(reported.observed_at(), at)?;
        for currency in [Currency::Krw, Currency::Usd] {
            if reported.valuation().amount(currency).is_some() {
                let price_date = calendar
                    .price_date(currency, reported.observed_at())
                    .map_err(ValuationSnapshotError::Snapshot)?;
                validate_price_date(price_date, snapshot.price_date(currency))?;
            }
        }
    }
    Ok(())
}

fn validate_observation_at(
    at: Timestamp,
    now: Timestamp,
) -> Result<Timestamp, ValuationSnapshotError> {
    if at > now {
        return Err(ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod));
    }
    Ok(at)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PriceRequirement {
    Required,
    Unused,
}
fn price_requirement(portfolio: &domain::Portfolio, currency: Currency) -> PriceRequirement {
    if portfolio
        .holdings()
        .iter()
        .any(|holding| holding.currency() == currency)
        || portfolio
            .reported()
            .and_then(|reported| reported.valuation().amount(currency))
            .is_some()
    {
        PriceRequirement::Required
    } else {
        PriceRequirement::Unused
    }
}
fn validate_price_basis(
    basis: SnapshotPriceBasis,
    requirement: PriceRequirement,
) -> Result<(), ValuationSnapshotError> {
    match (requirement, basis) {
        (
            PriceRequirement::Unused,
            SnapshotPriceBasis::CurrentSession
            | SnapshotPriceBasis::AwaitingSession
            | SnapshotPriceBasis::HolidayCarry,
        )
        | (PriceRequirement::Required, SnapshotPriceBasis::CurrentSession) => Ok(()),
        (
            PriceRequirement::Required,
            SnapshotPriceBasis::AwaitingSession | SnapshotPriceBasis::HolidayCarry,
        ) => Err(ValuationSnapshotError::PriceBasisDefinitionPending),
    }
}
fn validate_price_date(
    observed: CalendarDate,
    expected: CalendarDate,
) -> Result<(), ValuationSnapshotError> {
    if observed != expected {
        return Err(ValuationSnapshotError::PriceBasisDefinitionPending);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        PriceRequirement, ValuationSnapshotError, price_requirement, validate_observation_at,
        validate_portfolio_observations, validate_price_basis, validate_price_date,
    };
    use domain::{
        Nanosecond, OffsetSeconds, SnapshotError, SnapshotPriceBasis, Timestamp, UnixSeconds,
    };
    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(37),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    #[test]
    fn observation_is_preserved_before_or_at_now_and_rejected_in_the_future() {
        assert_eq!(validate_observation_at(at(11), at(17)), Ok(at(11)));
        assert_eq!(validate_observation_at(at(17), at(17)), Ok(at(17)));
        assert_eq!(
            validate_observation_at(at(23), at(17)),
            Err(ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod))
        );
    }
    #[test]
    fn used_prices_require_current_session_and_unused_currencies_do_not() {
        assert_eq!(
            validate_price_basis(
                SnapshotPriceBasis::CurrentSession,
                PriceRequirement::Required
            ),
            Ok(())
        );
        for basis in [
            SnapshotPriceBasis::AwaitingSession,
            SnapshotPriceBasis::HolidayCarry,
        ] {
            assert_eq!(
                validate_price_basis(basis, PriceRequirement::Unused),
                Ok(())
            );
            assert_eq!(
                validate_price_basis(basis, PriceRequirement::Required),
                Err(ValuationSnapshotError::PriceBasisDefinitionPending)
            );
        }
    }
    fn money(text: &str, currency: domain::Currency) -> domain::Money {
        domain::Money::parse(
            &domain::NumericInput::try_new(text).expect("입력"),
            currency,
        )
        .expect("금액")
    }
    fn holding(currency: domain::Currency) -> domain::Holding {
        holding_at(currency, at(17))
    }
    fn holding_at(currency: domain::Currency, observed: Timestamp) -> domain::Holding {
        use domain::{
            CommonShare, Currency, KoreanMasterDetail, LiquidationTrading, ListingStatus,
            MasterCode, MasterInstrument, MasterInstrumentRow, MasterSecurityType, NxtTrading,
            TradingSuspension,
        };
        let (market, symbol, detail) = match currency {
            Currency::Krw => (
                "KOSPI",
                "005930",
                Some(KoreanMasterDetail {
                    liquidation: LiquidationTrading::Regular,
                    krx: TradingSuspension::Clear,
                    nxt: NxtTrading::Unsupported,
                }),
            ),
            Currency::Usd => ("NASDAQ", "AAPL", None),
        };
        let master = MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new(market).expect("시장"),
            instrument_id: domain::InstrumentId::try_new(symbol).expect("ID"),
            symbol: domain::Symbol::try_new(symbol).expect("심볼"),
            name: domain::DisplayInstrumentName::try_new("고정 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("종류"),
            ),
            currency,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: detail,
            observed_at: observed,
        })
        .expect("마스터");
        domain::Holding::new(
            master.order_instrument().expect("종목"),
            domain::Quantity::parse(&domain::NumericInput::try_new("3").expect("입력"))
                .expect("수량"),
            domain::AcquisitionPrice::try_new(money("11", currency)).expect("원가"),
            domain::HoldingObservation::new(
                Some(domain::Price::try_new(money("17", currency)).expect("가격")),
                None,
                observed,
            ),
        )
        .expect("보유")
    }
    fn portfolio(holdings: Vec<domain::Holding>) -> domain::Portfolio {
        domain::Portfolio::new(
            holdings,
            domain::BuyingPower::new(money("137", domain::Currency::Krw), at(17)),
            domain::BuyingPower::new(money("29", domain::Currency::Usd), at(17)),
        )
        .expect("포트")
    }
    #[test]
    fn price_requirement_uses_holdings_and_reported_valuation_presence_per_currency() {
        use domain::Currency::{Krw, Usd};
        for currency in [Krw, Usd] {
            assert_eq!(
                price_requirement(&portfolio(vec![]), currency),
                PriceRequirement::Unused
            );
        }
        for (held, unused) in [(Krw, Usd), (Usd, Krw)] {
            let portfolio = portfolio(vec![holding(held)]);
            assert_eq!(
                price_requirement(&portfolio, held),
                PriceRequirement::Required
            );
            assert_eq!(
                price_requirement(&portfolio, unused),
                PriceRequirement::Unused
            );
        }
        for usd in [None, Some(money("0", Usd))] {
            let valuation = domain::CurrencyAmounts::new(money("13", Krw), usd).expect("평가");
            let purchase = domain::CurrencyAmounts::new(money("19", Krw), Some(money("23", Usd)))
                .expect("원금");
            let profit = domain::CurrencyAmounts::new(money("31", Krw), None).expect("손익");
            let portfolio = portfolio(vec![]).with_reported(domain::ReportedPortfolio::new(
                valuation,
                purchase,
                profit,
                at(17),
            ));
            assert_eq!(
                price_requirement(&portfolio, Krw),
                PriceRequirement::Required
            );
            assert_eq!(
                price_requirement(&portfolio, Usd),
                match usd {
                    Some(_) => PriceRequirement::Required,
                    None => PriceRequirement::Unused,
                }
            );
        }
    }
    #[test]
    fn price_date_must_match_in_both_directions() {
        let old = at(11).offset_calendar_date();
        let new = at(86417).offset_calendar_date();
        assert_eq!(validate_price_date(old, old), Ok(()));
        assert_eq!(
            validate_price_date(old, new),
            Err(ValuationSnapshotError::PriceBasisDefinitionPending)
        );
        assert_eq!(
            validate_price_date(new, old),
            Err(ValuationSnapshotError::PriceBasisDefinitionPending)
        );
    }
    fn session(start: i64, market: &domain::Market) -> domain::MarketSession {
        let data = Some(
            domain::SessionData::try_new(
                domain::TimeInterval::try_new(at(start), at(start.checked_add(10).expect("끝")))
                    .expect("구간"),
                None,
            )
            .expect("장"),
        );
        let info = match market {
            domain::Market::Kr => domain::SessionInformation::Kr {
                pre_market: None,
                regular_market: data,
                after_market: None,
            },
            domain::Market::Us | domain::Market::Unknown(_) => domain::SessionInformation::Us {
                day_market: None,
                pre_market: None,
                regular_market: data,
                after_market: None,
            },
        };
        domain::MarketSession::try_new(at(start).offset_calendar_date(), info).expect("세션")
    }
    fn calendar() -> domain::SnapshotCalendar {
        let make = |market: &domain::Market| {
            domain::MarketCalendar::try_new(
                session(-86400, market),
                session(0, market),
                session(86400, market),
            )
            .expect("달력")
        };
        domain::SnapshotCalendar::new(&make(&domain::Market::Kr), &make(&domain::Market::Us))
            .expect("평가 달력")
    }
    fn assert_observation_result(portfolio: &domain::Portfolio, expected: ValuationSnapshotError) {
        let calendar = calendar();
        let snapshot =
            domain::ValuationSnapshot::observe(&calendar, &portfolio.evaluate(None), at(17), None)
                .expect("평가");
        assert_eq!(
            validate_portfolio_observations(portfolio, &calendar, &snapshot, at(17)),
            Err(expected)
        );
    }
    #[test]
    fn holding_and_report_times_are_validated_independently() {
        for (time, expected) in [
            (at(5), ValuationSnapshotError::PriceBasisDefinitionPending),
            (
                at(23),
                ValuationSnapshotError::Snapshot(SnapshotError::WrongPeriod),
            ),
        ] {
            let holdings = portfolio(vec![holding_at(domain::Currency::Krw, time)]);
            assert_observation_result(&holdings, expected);
            let amounts = domain::CurrencyAmounts::new(money("13", domain::Currency::Krw), None)
                .expect("금액");
            let purchase = domain::CurrencyAmounts::new(
                money("19", domain::Currency::Krw),
                Some(money("23", domain::Currency::Usd)),
            )
            .expect("원금");
            let profit = domain::CurrencyAmounts::new(money("31", domain::Currency::Krw), None)
                .expect("손익");
            let report = domain::ReportedPortfolio::new(amounts, purchase, profit, time);
            let reported = portfolio(vec![holding(domain::Currency::Krw)]).with_reported(report);
            assert_observation_result(&reported, expected);
        }
    }
}
