use crate::estimates::codec::{
    Decoder, Encoder, invalid, put_currency, put_date, put_horizon, put_interval, put_money,
    put_news_source, put_price, put_ratio, put_timestamp, take_currency, take_date, take_horizon,
    take_interval, take_money, take_news_source, take_price, take_ratio, take_timestamp,
};
use crate::estimates::valuation_codec::{
    put_cash_failure, put_fx_rate, put_market, take_cash_failure, take_fx_rate, take_market,
};
use domain::{
    AcquisitionPrice, AuctionBoundary, CalibrationLedgerHead, CalibrationPosition, CollectedItem,
    Currency, CurrencyAmounts, DailyProfitLoss, DartReceiptNo, Disclosure, DisclosureSource,
    ExcludedSecurityType, HistoricalBuyingPower, HistoricalBuyingPowerState, HistoricalCashMoney,
    HistoricalClassifications, HistoricalHolding, HistoricalPortfolio, HistoricalPortfolioFacts,
    HistoricalSectorAssignment, HistoricalSecurityType, HistoricalSignalContext,
    HistoricalSignalQuantity, HoldingReturn, HoursPermissionState, InformationSource,
    InstrumentClassification, InstrumentId, InvestorProfile, KindReceiptNo, Market, MarketCalendar,
    MarketSession, MarketState, MasterInstrument, Money, NormalizedUrl, NumericInput, Policy,
    PolicyConcentration, PolicyPrinciple, PolicyText, PortfolioDailyProfitLoss, ProfileWeight,
    Quantity, RatioQuotient, ReceiptNo, ReportedHolding, ReportedPortfolio, SavedInvestorProfile,
    SecEdgarReceiptNo, SectorAssignmentReason, SectorId, SectorMembership, SessionData,
    SessionInformation, StoredPriceIdentity, StoredPriceObservation, Symbol, ToolReference,
    UntrustedText,
};

pub(super) fn put_hours_permission(out: &mut Encoder, state: HoursPermissionState) {
    out.push(match state {
        HoursPermissionState::Disabled => "disabled",
        HoursPermissionState::Enabled => "enabled",
    });
}

pub(super) fn take_hours_permission(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<HoursPermissionState> {
    match input.next()? {
        "disabled" => Ok(HoursPermissionState::Disabled),
        "enabled" => Ok(HoursPermissionState::Enabled),
        _ => Err(invalid(())),
    }
}

pub(super) fn put_stored_price_obs(out: &mut Encoder, value: StoredPriceObservation) {
    put_price(out, value.price);
    put_timestamp(out, value.observed_at);
    out.push(match value.identity {
        StoredPriceIdentity::Identified => "identified",
        StoredPriceIdentity::Unidentified => "unidentified",
    });
}

pub(super) fn take_stored_price_obs(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<StoredPriceObservation> {
    let price = take_price(input)?;
    let observed_at = take_timestamp(input)?;
    let identity = match input.next()? {
        "identified" => StoredPriceIdentity::Identified,
        "unidentified" => StoredPriceIdentity::Unidentified,
        _ => return Err(invalid(())),
    };
    Ok(StoredPriceObservation {
        price,
        observed_at,
        identity,
    })
}

pub(super) fn put_disclosure_source(out: &mut Encoder, source: &DisclosureSource) {
    out.push(match source {
        DisclosureSource::Kind => "kind",
        DisclosureSource::Dart => "dart",
        DisclosureSource::SecEdgar => "sec_edgar",
    });
}

pub(super) fn take_disclosure_source(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<DisclosureSource> {
    match input.next()? {
        "kind" => Ok(DisclosureSource::Kind),
        "dart" => Ok(DisclosureSource::Dart),
        "sec_edgar" => Ok(DisclosureSource::SecEdgar),
        _ => Err(invalid(())),
    }
}

pub(super) fn put_collected_item(out: &mut Encoder, item: &CollectedItem) {
    match item.source() {
        InformationSource::Web => out.push("web"),
        InformationSource::News(source) => {
            out.push("news");
            put_news_source(out, source);
        }
        InformationSource::Disclosure(source) => {
            out.push("disclosure");
            put_disclosure_source(out, source);
        }
    }
    out.push(item.url().as_str());
    out.push(item.title().as_untrusted_str());
    match item.body() {
        Some(body) => {
            out.push("some");
            out.push(body.as_untrusted_str());
        }
        None => out.push("none"),
    }
}

pub(super) fn take_collected_item(input: &mut Decoder<'_>) -> rusqlite::Result<CollectedItem> {
    let source = match input.next()? {
        "web" => InformationSource::Web,
        "news" => InformationSource::News(take_news_source(input)?),
        "disclosure" => InformationSource::Disclosure(take_disclosure_source(input)?),
        _ => return Err(invalid(())),
    };
    let url = NormalizedUrl::try_new(input.next()?).map_err(invalid)?;
    let title = UntrustedText::try_new(input.next()?).map_err(invalid)?;
    let body = match input.next()? {
        "some" => Some(UntrustedText::try_new(input.next()?).map_err(invalid)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    Ok(CollectedItem::from_saved_source(source, url, title, body))
}

pub(super) fn put_disclosure(out: &mut Encoder, item: &Disclosure) {
    match item.key().receipt_no() {
        ReceiptNo::Dart(value) => {
            out.push("dart");
            out.push(value.as_str());
        }
        ReceiptNo::Kind(value) => {
            out.push("kind");
            out.push(value.as_str());
        }
        ReceiptNo::SecEdgar(value) => {
            out.push("sec");
            out.push(value.as_str());
        }
    }
    let collected = item.collected();
    out.push(collected.url().as_str());
    out.push(collected.title().as_untrusted_str());
    match collected.body() {
        Some(body) => {
            out.push("some");
            out.push(body.as_untrusted_str());
        }
        None => out.push("none"),
    }
}

pub(super) fn take_disclosure(input: &mut Decoder<'_>) -> rusqlite::Result<Disclosure> {
    let receipt = match input.next()? {
        "dart" => ReceiptNo::Dart(DartReceiptNo::try_new(input.next()?).map_err(invalid)?),
        "kind" => ReceiptNo::Kind(KindReceiptNo::try_new(input.next()?).map_err(invalid)?),
        "sec" => ReceiptNo::SecEdgar(SecEdgarReceiptNo::try_new(input.next()?).map_err(invalid)?),
        _ => return Err(invalid(())),
    };
    let url = NormalizedUrl::try_new(input.next()?).map_err(invalid)?;
    let title = UntrustedText::try_new(input.next()?).map_err(invalid)?;
    let body = match input.next()? {
        "some" => Some(UntrustedText::try_new(input.next()?).map_err(invalid)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    Ok(Disclosure::new(receipt, url, title, body))
}

fn put_excluded_security_type(out: &mut Encoder, value: ExcludedSecurityType) {
    out.push(match value {
        ExcludedSecurityType::Stock => "stock",
        ExcludedSecurityType::ForeignStock => "foreign_stock",
        ExcludedSecurityType::DepositaryReceipt => "depositary_receipt",
        ExcludedSecurityType::InfrastructureFund => "infrastructure_fund",
        ExcludedSecurityType::Reit => "reit",
        ExcludedSecurityType::Etf => "etf",
        ExcludedSecurityType::ForeignEtf => "foreign_etf",
        ExcludedSecurityType::Etn => "etn",
        ExcludedSecurityType::StockWarrants => "stock_warrants",
    });
}

fn take_excluded_security_type(input: &mut Decoder<'_>) -> rusqlite::Result<ExcludedSecurityType> {
    match input.next()? {
        "stock" => Ok(ExcludedSecurityType::Stock),
        "foreign_stock" => Ok(ExcludedSecurityType::ForeignStock),
        "depositary_receipt" => Ok(ExcludedSecurityType::DepositaryReceipt),
        "infrastructure_fund" => Ok(ExcludedSecurityType::InfrastructureFund),
        "reit" => Ok(ExcludedSecurityType::Reit),
        "etf" => Ok(ExcludedSecurityType::Etf),
        "foreign_etf" => Ok(ExcludedSecurityType::ForeignEtf),
        "etn" => Ok(ExcludedSecurityType::Etn),
        "stock_warrants" => Ok(ExcludedSecurityType::StockWarrants),
        _ => Err(invalid(())),
    }
}

pub(crate) fn put_investor_profile(out: &mut Encoder, profile: &InvestorProfile) {
    let saved = profile.saved_state();
    put_horizon(out, domain::Horizon::new(saved.horizon));
    put_hours_permission(out, saved.hours_permission);
    crate::estimates::codec::put_appetite(out, saved.risk_appetite);
    out.push_usize(saved.sectors.len());
    for sector in &saved.sectors {
        out.push(sector.as_str());
    }
    out.push_usize(saved.security_types.len());
    for kind in &saved.security_types {
        put_excluded_security_type(out, *kind);
    }
    put_ratio(out, saved.instrument_cap);
    put_ratio(out, saved.instrument_warning);
    put_ratio(out, saved.sector_cap);
}

pub(crate) fn take_investor_profile(input: &mut Decoder<'_>) -> rusqlite::Result<InvestorProfile> {
    let horizon = take_horizon(input)?.selection();
    let hours_permission = take_hours_permission(input)?;
    let risk_appetite = crate::estimates::codec::take_appetite(input)?;
    let sector_count = input.next_usize()?;
    let mut sectors = Vec::with_capacity(sector_count);
    for _ in 0..sector_count {
        sectors.push(SectorId::try_new(input.next()?).map_err(invalid)?);
    }
    let type_count = input.next_usize()?;
    let mut security_types = Vec::with_capacity(type_count);
    for _ in 0..type_count {
        security_types.push(take_excluded_security_type(input)?);
    }
    let instrument_cap = take_ratio(input)?;
    let instrument_warning = take_ratio(input)?;
    let sector_cap = take_ratio(input)?;
    InvestorProfile::restore_from_saved_state(SavedInvestorProfile {
        horizon,
        hours_permission,
        risk_appetite,
        sectors,
        security_types,
        instrument_cap,
        instrument_warning,
        sector_cap,
    })
    .map_err(invalid)
}

fn put_policy(out: &mut Encoder, policy: &Policy) {
    put_investor_profile(out, policy.profile());
    out.push_usize(policy.unresolved_principles().len());
    for principle in policy.unresolved_principles() {
        out.push(principle.description().as_str());
    }
    out.push_usize(policy.context().len());
    for entry in policy.context() {
        out.push(entry.as_str());
    }
    match policy.minimum_cash() {
        Some(weight) => {
            out.push("some");
            put_ratio(out, weight.as_ratio());
        }
        None => out.push("none"),
    }
    out.push(match policy.concentration() {
        PolicyConcentration::Warning => "warning",
        PolicyConcentration::Clear => "clear",
        PolicyConcentration::NotApplicable => "not_applicable",
        PolicyConcentration::Unavailable => "unavailable",
    });
}

fn take_policy(input: &mut Decoder<'_>) -> rusqlite::Result<Policy> {
    let mut policy = Policy::new(take_investor_profile(input)?);
    let unresolved_count = input.next_usize()?;
    for _ in 0..unresolved_count {
        let text = PolicyText::try_new(input.next()?).map_err(invalid)?;
        policy = policy.with_unresolved_principle(PolicyPrinciple::from_missing_contract(text));
    }
    let context_count = input.next_usize()?;
    for _ in 0..context_count {
        let text = PolicyText::try_new(input.next()?).map_err(invalid)?;
        policy = policy.with_context(text);
    }
    if match input.next()? {
        "some" => true,
        "none" => false,
        _ => return Err(invalid(())),
    } {
        let weight = ProfileWeight::try_new(take_ratio(input)?).map_err(invalid)?;
        policy = policy.with_minimum_cash(weight);
    }
    let concentration = match input.next()? {
        "warning" => PolicyConcentration::Warning,
        "clear" => PolicyConcentration::Clear,
        "not_applicable" => PolicyConcentration::NotApplicable,
        "unavailable" => PolicyConcentration::Unavailable,
        _ => return Err(invalid(())),
    };
    Ok(policy.with_concentration(concentration))
}

fn put_sector_assignment(out: &mut Encoder, entry: &HistoricalSectorAssignment) {
    put_market(out, &entry.market);
    out.push(entry.symbol.as_str());
    match &entry.classification {
        InstrumentClassification::Sector(id) => {
            out.push("sector");
            out.push(id.as_str());
        }
        InstrumentClassification::EtfFund => out.push("etf_fund"),
        InstrumentClassification::Unclassified => out.push("unclassified"),
    }
    out.push(match entry.membership {
        None => "none",
        Some(SectorMembership::Single) => "single",
        Some(SectorMembership::Multiple) => "multiple",
    });
    out.push(match entry.reason {
        None => "none",
        Some(SectorAssignmentReason::SingleMajorSector) => "single_major_sector",
        Some(SectorAssignmentReason::LargestMajorCapitalization) => "largest_major_cap",
        Some(SectorAssignmentReason::CapitalizationUnavailable) => "cap_unavailable",
        Some(SectorAssignmentReason::EtfFundFromMaster) => "etf_fund_from_master",
        Some(SectorAssignmentReason::NoMajorMembership) => "no_major_membership",
        Some(SectorAssignmentReason::UnknownSecurityType) => "unknown_security_type",
    });
}

fn take_sector_assignment(input: &mut Decoder<'_>) -> rusqlite::Result<HistoricalSectorAssignment> {
    let market = take_market(input)?;
    let symbol = Symbol::try_new(input.next()?).map_err(invalid)?;
    let classification = match input.next()? {
        "sector" => {
            InstrumentClassification::Sector(SectorId::try_new(input.next()?).map_err(invalid)?)
        }
        "etf_fund" => InstrumentClassification::EtfFund,
        "unclassified" => InstrumentClassification::Unclassified,
        _ => return Err(invalid(())),
    };
    let membership = match input.next()? {
        "none" => None,
        "single" => Some(SectorMembership::Single),
        "multiple" => Some(SectorMembership::Multiple),
        _ => return Err(invalid(())),
    };
    let reason = match input.next()? {
        "none" => None,
        "single_major_sector" => Some(SectorAssignmentReason::SingleMajorSector),
        "largest_major_cap" => Some(SectorAssignmentReason::LargestMajorCapitalization),
        "cap_unavailable" => Some(SectorAssignmentReason::CapitalizationUnavailable),
        "etf_fund_from_master" => Some(SectorAssignmentReason::EtfFundFromMaster),
        "no_major_membership" => Some(SectorAssignmentReason::NoMajorMembership),
        "unknown_security_type" => Some(SectorAssignmentReason::UnknownSecurityType),
        _ => return Err(invalid(())),
    };
    Ok(HistoricalSectorAssignment {
        market,
        symbol,
        classification,
        membership,
        reason,
    })
}

fn put_classifications(out: &mut Encoder, value: &HistoricalClassifications) {
    out.push_usize(value.sectors.len());
    for entry in &value.sectors {
        put_sector_assignment(out, entry);
    }
    out.push_usize(value.security.len());
    for entry in &value.security {
        put_market(out, &entry.market);
        out.push(entry.symbol.as_str());
        put_excluded_security_type(out, entry.security_type);
    }
}

fn take_classifications(input: &mut Decoder<'_>) -> rusqlite::Result<HistoricalClassifications> {
    let sector_count = input.next_usize()?;
    let mut sectors = Vec::with_capacity(sector_count);
    for _ in 0..sector_count {
        sectors.push(take_sector_assignment(input)?);
    }
    let security_count = input.next_usize()?;
    let mut security = Vec::with_capacity(security_count);
    for _ in 0..security_count {
        security.push(HistoricalSecurityType {
            market: take_market(input)?,
            symbol: Symbol::try_new(input.next()?).map_err(invalid)?,
            security_type: take_excluded_security_type(input)?,
        });
    }
    Ok(HistoricalClassifications { sectors, security })
}

fn put_session_data(out: &mut Encoder, session: SessionData) {
    put_interval(out, session.interval());
    match session.auction() {
        None => out.push("none"),
        Some(AuctionBoundary::StartsAt(at)) => {
            out.push("starts");
            put_timestamp(out, at);
        }
        Some(AuctionBoundary::EndsAt(at)) => {
            out.push("ends");
            put_timestamp(out, at);
        }
    }
}

fn take_session_data(input: &mut Decoder<'_>) -> rusqlite::Result<SessionData> {
    let interval = take_interval(input)?;
    let auction = match input.next()? {
        "none" => None,
        "starts" => Some(AuctionBoundary::StartsAt(take_timestamp(input)?)),
        "ends" => Some(AuctionBoundary::EndsAt(take_timestamp(input)?)),
        _ => return Err(invalid(())),
    };
    SessionData::try_new(interval, auction).map_err(invalid)
}

fn put_market_session(out: &mut Encoder, session: &MarketSession) {
    put_market(out, &session.market());
    put_date(out, session.date());
    out.push_usize(session.sessions().len());
    for segment in session.sessions() {
        out.push(match segment.state() {
            MarketState::DayMarket => "day",
            MarketState::PreMarket => "pre",
            MarketState::RegularMarket => "regular",
            MarketState::AfterMarket => "after",
            MarketState::Closed => "closed",
            MarketState::Unknown => "unknown",
        });
        put_session_data(out, segment.session());
    }
}

fn take_market_session(input: &mut Decoder<'_>) -> rusqlite::Result<MarketSession> {
    let market = take_market(input)?;
    let date = take_date(input)?;
    let count = input.next_usize()?;
    let mut day_market = None;
    let mut pre_market = None;
    let mut regular_market = None;
    let mut after_market = None;
    for _ in 0..count {
        let kind = input.next()?;
        let segment = take_session_data(input)?;
        match kind {
            "day" => day_market = Some(segment),
            "pre" => pre_market = Some(segment),
            "regular" => regular_market = Some(segment),
            "after" => after_market = Some(segment),
            _ => return Err(invalid(())),
        }
    }
    let info = match market {
        Market::Kr => {
            if day_market.is_some() {
                return Err(invalid(()));
            }
            SessionInformation::Kr {
                pre_market,
                regular_market,
                after_market,
            }
        }
        Market::Us => SessionInformation::Us {
            day_market,
            pre_market,
            regular_market,
            after_market,
        },
        Market::Unknown(_) => return Err(invalid(())),
    };
    MarketSession::try_new(date, info).map_err(invalid)
}

fn put_market_calendar(out: &mut Encoder, calendar: &MarketCalendar) {
    put_market_session(out, calendar.previous_business_day());
    put_market_session(out, calendar.today());
    put_market_session(out, calendar.next_business_day());
}

fn take_market_calendar(input: &mut Decoder<'_>) -> rusqlite::Result<MarketCalendar> {
    let previous = take_market_session(input)?;
    let today = take_market_session(input)?;
    let next = take_market_session(input)?;
    MarketCalendar::try_new(previous, today, next).map_err(invalid)
}

fn put_currency_amounts(out: &mut Encoder, amounts: CurrencyAmounts) {
    if let Some(krw) = amounts.amount(Currency::Krw) {
        put_money(out, krw);
    }
    match amounts.amount(Currency::Usd) {
        Some(usd) => {
            out.push("some");
            put_money(out, usd);
        }
        None => out.push("none"),
    }
}

fn take_currency_amounts(input: &mut Decoder<'_>) -> rusqlite::Result<CurrencyAmounts> {
    let krw = take_money(input)?;
    let usd = match input.next()? {
        "some" => Some(take_money(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    CurrencyAmounts::new(krw, usd).map_err(invalid)
}

fn put_reported_portfolio(out: &mut Encoder, reported: ReportedPortfolio) {
    put_currency_amounts(out, reported.valuation());
    put_currency_amounts(out, reported.purchase());
    put_currency_amounts(out, reported.profit());
    put_timestamp(out, reported.observed_at());
    match reported.daily_profit_loss() {
        Some(daily) => {
            out.push("some");
            put_currency_amounts(out, daily.amounts());
            put_ratio(out, daily.rate());
        }
        None => out.push("none"),
    }
    match reported.return_rate() {
        Some(obs) => match obs.rate() {
            HoldingReturn::Defined(RatioQuotient::Exact(rate)) => {
                out.push("some");
                put_ratio(out, rate);
            }
            HoldingReturn::Defined(RatioQuotient::Approximate(_)) | HoldingReturn::ZeroCost => {
                out.push("none");
            }
        },
        None => out.push("none"),
    }
}

fn take_reported_portfolio(input: &mut Decoder<'_>) -> rusqlite::Result<ReportedPortfolio> {
    let valuation = take_currency_amounts(input)?;
    let purchase = take_currency_amounts(input)?;
    let profit = take_currency_amounts(input)?;
    let observed_at = take_timestamp(input)?;
    let mut reported = ReportedPortfolio::new(valuation, purchase, profit, observed_at);
    if match input.next()? {
        "some" => true,
        "none" => false,
        _ => return Err(invalid(())),
    } {
        let amounts = take_currency_amounts(input)?;
        let rate = take_ratio(input)?;
        reported = reported.with_daily_profit_loss(PortfolioDailyProfitLoss::new(amounts, rate));
    }
    if match input.next()? {
        "some" => true,
        "none" => false,
        _ => return Err(invalid(())),
    } {
        reported = reported.with_return_rate(take_ratio(input)?);
    }
    Ok(reported)
}

fn put_historical_buying_power(out: &mut Encoder, value: HistoricalBuyingPower) {
    put_currency(out, value.currency);
    put_timestamp(out, value.observed_at);
    match value.state {
        HistoricalBuyingPowerState::Available(cash) => {
            out.push("available");
            put_currency(out, cash.currency());
            out.push(cash.display_text().as_str());
        }
        HistoricalBuyingPowerState::NegativeBuyingPower => out.push("negative"),
        HistoricalBuyingPowerState::CashLookupFailed(failure) => {
            out.push("failed");
            put_cash_failure(out, failure);
        }
    }
}

fn take_historical_buying_power(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<HistoricalBuyingPower> {
    let currency = take_currency(input)?;
    let observed_at = take_timestamp(input)?;
    let state = match input.next()? {
        "available" => {
            let cash_currency = take_currency(input)?;
            let numeric = NumericInput::try_new(input.next()?).map_err(invalid)?;
            let money = Money::parse(&numeric, cash_currency).map_err(invalid)?;
            HistoricalBuyingPowerState::Available(
                HistoricalCashMoney::try_new(money).map_err(invalid)?,
            )
        }
        "negative" => HistoricalBuyingPowerState::NegativeBuyingPower,
        "failed" => HistoricalBuyingPowerState::CashLookupFailed(take_cash_failure(input)?),
        _ => return Err(invalid(())),
    };
    Ok(HistoricalBuyingPower {
        currency,
        state,
        observed_at,
    })
}

fn put_historical_holding(out: &mut Encoder, item: &HistoricalHolding) {
    out.push(item.instrument_id.as_str());
    put_market(out, &item.market);
    out.push(item.symbol.as_str());
    out.push(item.quantity.display_text().as_str());
    put_money(out, item.acquisition_price.money());
    match item.current_price {
        Some(price) => {
            out.push("some");
            put_price(out, price);
        }
        None => out.push("none"),
    }
    match item.reported {
        Some(reported) => {
            out.push("some");
            put_money(out, reported.valuation());
            put_money(out, reported.profit());
            put_ratio(out, reported.return_rate());
            match reported.daily_profit_loss() {
                Some(daily) => {
                    out.push("some");
                    put_money(out, daily.amount());
                    put_ratio(out, daily.rate());
                }
                None => out.push("none"),
            }
        }
        None => out.push("none"),
    }
    put_timestamp(out, item.observed_at);
}

fn take_historical_holding(input: &mut Decoder<'_>) -> rusqlite::Result<HistoricalHolding> {
    let instrument_id = InstrumentId::try_new(input.next()?).map_err(invalid)?;
    let market = take_market(input)?;
    let symbol = Symbol::try_new(input.next()?).map_err(invalid)?;
    let qty_input = NumericInput::try_new(input.next()?).map_err(invalid)?;
    let quantity = HistoricalSignalQuantity::new(Quantity::parse(&qty_input).map_err(invalid)?);
    let acquisition_price = AcquisitionPrice::try_new(take_money(input)?).map_err(invalid)?;
    let current_price = match input.next()? {
        "some" => Some(take_price(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let reported = match input.next()? {
        "some" => {
            let mut item =
                ReportedHolding::new(take_money(input)?, take_money(input)?, take_ratio(input)?)
                    .map_err(invalid)?;
            if match input.next()? {
                "some" => true,
                "none" => false,
                _ => return Err(invalid(())),
            } {
                let daily = DailyProfitLoss::new(take_money(input)?, take_ratio(input)?);
                item = item.with_daily_profit_loss(daily).map_err(invalid)?;
            }
            Some(item)
        }
        "none" => None,
        _ => return Err(invalid(())),
    };
    let observed_at = take_timestamp(input)?;
    Ok(HistoricalHolding {
        instrument_id,
        market,
        symbol,
        quantity,
        acquisition_price,
        current_price,
        reported,
        observed_at,
    })
}

fn put_historical_portfolio(out: &mut Encoder, portfolio: &HistoricalPortfolio) {
    let facts = portfolio.facts();
    out.push_usize(facts.holdings.len());
    for holding in &facts.holdings {
        put_historical_holding(out, holding);
    }
    put_historical_buying_power(out, facts.krw);
    put_historical_buying_power(out, facts.usd);
    match facts.reported {
        Some(reported) => {
            out.push("some");
            put_reported_portfolio(out, reported);
        }
        None => out.push("none"),
    }
}

fn take_historical_portfolio(
    input: &mut Decoder<'_>,
    masters: &[MasterInstrument],
) -> rusqlite::Result<HistoricalPortfolio> {
    let count = input.next_usize()?;
    let mut holdings = Vec::with_capacity(count);
    for _ in 0..count {
        holdings.push(take_historical_holding(input)?);
    }
    let krw = take_historical_buying_power(input)?;
    let usd = take_historical_buying_power(input)?;
    let reported = match input.next()? {
        "some" => Some(take_reported_portfolio(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    HistoricalPortfolio::from_saved_facts(
        HistoricalPortfolioFacts {
            holdings,
            krw,
            usd,
            reported,
        },
        masters,
    )
    .map_err(invalid)
}

pub(super) fn put_signal_context(out: &mut Encoder, ctx: &HistoricalSignalContext) {
    match &ctx.portfolio {
        Some(portfolio) => {
            out.push("some");
            put_historical_portfolio(out, portfolio);
        }
        None => out.push("none"),
    }
    match ctx.rate {
        Some(rate) => {
            out.push("some");
            put_fx_rate(out, rate);
        }
        None => out.push("none"),
    }
    put_policy(out, &ctx.policy);
    put_classifications(out, &ctx.classifications);
    out.push_usize(ctx.items.len());
    for item in &ctx.items {
        put_collected_item(out, item);
    }
    out.push_usize(ctx.disclosures.len());
    for disclosure in &ctx.disclosures {
        put_disclosure(out, disclosure);
    }
    out.push_usize(ctx.tools.len());
    for tool in &ctx.tools {
        out.push(tool.as_str());
    }
    out.push_usize(ctx.calendars.len());
    for calendar in &ctx.calendars {
        put_market_calendar(out, calendar);
    }
    put_timestamp(out, ctx.evaluated_at);
    match ctx.target {
        Some(target) => {
            out.push("some");
            put_stored_price_obs(out, target);
        }
        None => out.push("none"),
    }
    match &ctx.calibration_head {
        Some(head) => {
            out.push("some");
            crate::estimates::codec::put_calibration_key(out, head.key());
            out.push_u64(*head.position().as_u64());
        }
        None => out.push("none"),
    }
}

struct ContextCollections {
    items: Vec<CollectedItem>,
    disclosures: Vec<Disclosure>,
    tools: Vec<ToolReference>,
    calendars: Vec<MarketCalendar>,
}

fn take_signal_context_collections(
    input: &mut Decoder<'_>,
) -> rusqlite::Result<ContextCollections> {
    let item_count = input.next_usize()?;
    let mut items = Vec::with_capacity(item_count);
    for _ in 0..item_count {
        items.push(take_collected_item(input)?);
    }
    let disclosure_count = input.next_usize()?;
    let mut disclosures = Vec::with_capacity(disclosure_count);
    for _ in 0..disclosure_count {
        disclosures.push(take_disclosure(input)?);
    }
    let tool_count = input.next_usize()?;
    let mut tools = Vec::with_capacity(tool_count);
    for _ in 0..tool_count {
        tools.push(ToolReference::try_new(input.next()?).map_err(invalid)?);
    }
    let cal_count = input.next_usize()?;
    let mut calendars = Vec::with_capacity(cal_count);
    for _ in 0..cal_count {
        calendars.push(take_market_calendar(input)?);
    }
    Ok(ContextCollections {
        items,
        disclosures,
        tools,
        calendars,
    })
}

pub(super) fn take_signal_context(
    input: &mut Decoder<'_>,
    masters: &[MasterInstrument],
) -> rusqlite::Result<HistoricalSignalContext> {
    let portfolio = match input.next()? {
        "some" => Some(take_historical_portfolio(input, masters)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let rate = match input.next()? {
        "some" => Some(take_fx_rate(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let policy = take_policy(input)?;
    let classifications = take_classifications(input)?;
    let collections = take_signal_context_collections(input)?;
    let evaluated_at = take_timestamp(input)?;
    let target = match input.next()? {
        "some" => Some(take_stored_price_obs(input)?),
        "none" => None,
        _ => return Err(invalid(())),
    };
    let calibration_head = match input.next()? {
        "some" => {
            let key = crate::estimates::codec::take_calibration_key(input)?;
            let pos = CalibrationPosition::new(input.next_u64()?);
            Some(CalibrationLedgerHead::new(key, pos))
        }
        "none" => None,
        _ => return Err(invalid(())),
    };
    Ok(HistoricalSignalContext {
        portfolio,
        rate,
        policy,
        classifications,
        items: collections.items,
        disclosures: collections.disclosures,
        tools: collections.tools,
        calendars: collections.calendars,
        evaluated_at,
        target,
        calibration_head,
    })
}
