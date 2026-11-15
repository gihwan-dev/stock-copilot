use crate::{
    KnownMarket, MarketDataError, MarketObservation, MasterInstrument, QuoteInstrument, SectorId,
    Symbol, Timestamp,
};
use std::collections::BTreeSet;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorCompositionScope {
    IncludingDescendants,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorComposition {
    sector: SectorId,
    market: KnownMarket,
    symbols: Vec<Symbol>,
    stocks: Vec<SectorMember>,
    observation: MarketObservation,
    scope: SectorCompositionScope,
}
impl SectorComposition {
    pub fn from_stocks_response(
        sector: SectorId,
        market: KnownMarket,
        rows: Vec<SectorStockRow>,
        collected: Timestamp,
    ) -> Result<Self, MarketDataError> {
        let mut seen = BTreeSet::new();
        let mut symbols = Vec::new();
        let mut stocks = Vec::new();
        for row in rows {
            let symbol = &row.symbol;
            crate::market_ranking::validate_market_symbol(symbol, market)?;
            if !seen.insert(symbol.clone()) {
                return Err(MarketDataError::DuplicateInstrument);
            }
            let member = sector_member(row)?;
            symbols.push(member.symbol.clone());
            stocks.push(member);
        }
        Ok(Self {
            sector,
            market,
            symbols,
            stocks,
            observation: MarketObservation::collected_only(collected),
            scope: SectorCompositionScope::IncludingDescendants,
        })
    }
    #[must_use]
    pub const fn sector(&self) -> &SectorId {
        &self.sector
    }
    #[must_use]
    pub const fn market(&self) -> KnownMarket {
        self.market
    }
    #[must_use]
    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }
    #[must_use]
    pub fn stocks(&self) -> &[SectorMember] {
        &self.stocks
    }
    #[must_use]
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
    #[must_use]
    pub const fn scope(&self) -> SectorCompositionScope {
        self.scope
    }
    pub fn identified_member(
        &self,
        master: &MasterInstrument,
    ) -> Result<QuoteInstrument, MarketDataError> {
        let identified = master
            .quote_instrument()
            .or(Err(MarketDataError::InvalidSymbol))?;
        if identified.market() != &self.market || !self.symbols.contains(identified.symbol()) {
            return Err(MarketDataError::InvalidSymbol);
        }
        Ok(identified)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorStockRow {
    pub symbol: Symbol,
    pub currency: crate::Currency,
    pub market_capitalization: Option<crate::NumericInput>,
    pub trading_amount: Option<crate::NumericInput>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SectorAmount {
    Unavailable,
    Available(crate::Money),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorMember {
    symbol: Symbol,
    market_capitalization: SectorAmount,
    trading_amount: SectorAmount,
}
impl SectorMember {
    #[must_use]
    pub const fn symbol(&self) -> &Symbol {
        &self.symbol
    }
    #[must_use]
    pub const fn market_capitalization(&self) -> SectorAmount {
        self.market_capitalization
    }
    #[must_use]
    pub const fn trading_amount(&self) -> SectorAmount {
        self.trading_amount
    }
}
fn sector_member(row: SectorStockRow) -> Result<SectorMember, MarketDataError> {
    if row.currency != crate::Currency::Krw {
        return Err(MarketDataError::InvalidCurrency);
    }
    Ok(SectorMember {
        symbol: row.symbol,
        market_capitalization: sector_amount(row.market_capitalization.as_ref())?,
        trading_amount: sector_amount(row.trading_amount.as_ref())?,
    })
}
pub(crate) fn sector_amount(
    input: Option<&crate::NumericInput>,
) -> Result<SectorAmount, MarketDataError> {
    match input {
        None => Ok(SectorAmount::Unavailable),
        Some(raw) => {
            let value = crate::Money::parse(raw, crate::Currency::Krw)
                .or(Err(MarketDataError::InvalidNumber))?;
            if value.amount().as_decimal() < rust_decimal::Decimal::ZERO {
                return Err(MarketDataError::InvalidNumber);
            }
            Ok(SectorAmount::Available(value))
        }
    }
}
impl std::fmt::Debug for SectorAmount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "Unavailable",
            Self::Available(_) => "Available([REDACTED])",
        })
    }
}
#[cfg(test)]
mod tests {
    use crate::{
        KnownMarket, Nanosecond, ObservationAlignment, OffsetSeconds, SectorComposition,
        SectorCompositionScope, SectorId, Symbol, Timestamp, UnixSeconds,
    };
    fn rows(symbols: &[Symbol]) -> Vec<crate::SectorStockRow> {
        symbols
            .iter()
            .map(|symbol| crate::SectorStockRow {
                symbol: symbol.clone(),
                currency: crate::Currency::Krw,
                market_capitalization: None,
                trading_amount: None,
            })
            .collect()
    }
    #[test]
    fn sector_stocks_preserve_direction_and_unknown_server_time() {
        let stamp = Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        let symbols = vec![
            Symbol::try_new("005930").expect("종목"),
            Symbol::try_new("000660").expect("종목"),
        ];
        let value = SectorComposition::from_stocks_response(
            SectorId::try_new("100").expect("섹터"),
            KnownMarket::Kr,
            rows(&symbols),
            stamp,
        )
        .expect("구성");
        assert_eq!(value.sector().as_str(), "100");
        assert_eq!(value.market(), KnownMarket::Kr);
        assert_eq!(value.symbols(), symbols);
        assert_eq!(
            value.observation().alignment(),
            ObservationAlignment::Unknown
        );
        assert_eq!(value.scope(), SectorCompositionScope::IncludingDescendants);
        SectorComposition::from_stocks_response(
            SectorId::try_new("100").expect("섹터"),
            KnownMarket::Kr,
            rows(&vec![symbols.first().expect("종목").clone(); 2]),
            stamp,
        )
        .expect_err("거부");
    }

    fn master(symbol: &str, listing: &str) -> crate::MasterInstrument {
        crate::MasterInstrument::try_new(crate::MasterInstrumentRow {
            listing_market: crate::MasterCode::try_new(listing).expect("상장 시장"),
            symbol: Symbol::try_new(symbol).expect("심볼"),
            instrument_id: crate::InstrumentId::try_new("stable").expect("ID"),
            name: crate::DisplayInstrumentName::try_new("종목").expect("이름"),
            security_type: crate::MasterSecurityType::from_code(
                crate::MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: crate::Currency::Krw,
            listing_status: crate::ListingStatus::Active,
            common_share: crate::CommonShare::Common,
            korean_detail: None,
            observed_at: Timestamp::from_parts(
                UnixSeconds::new(0),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("시각"),
        })
        .expect("마스터")
    }
    #[test]
    fn sector_member_is_identified_only_by_matching_master() {
        let stamp = Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        let input = master("005930", "KOSPI");
        let composition = SectorComposition::from_stocks_response(
            SectorId::try_new("100").expect("섹터"),
            KnownMarket::Kr,
            rows(&[input.row().symbol.clone()]),
            stamp,
        )
        .expect("구성");
        assert_eq!(
            composition
                .identified_member(&input)
                .expect("일치")
                .instrument_id(),
            &input.row().instrument_id
        );
        composition
            .identified_member(&master("000660", "KOSDAQ"))
            .expect_err("미소속");
        composition
            .identified_member(&master("005930", "OTHER"))
            .expect_err("미지 시장");
        let foreign = SectorComposition::from_stocks_response(
            SectorId::try_new("100").expect("섹터"),
            KnownMarket::Us,
            rows(&[Symbol::try_new("AAPL").expect("종목")]),
            stamp,
        )
        .expect("구성");
        foreign.identified_member(&input).expect_err("시장 불일치");
        SectorComposition::from_stocks_response(
            SectorId::try_new("100").expect("섹터"),
            KnownMarket::Kr,
            rows(&[Symbol::try_new("AAPL").expect("종목")]),
            stamp,
        )
        .expect_err("심볼 불일치");
    }

    #[test]
    fn composition_preserves_market_cap_and_trading_amount_including_null() {
        let stamp = Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        let input = || crate::SectorStockRow {
            symbol: Symbol::try_new("ABCDEF").expect("종목"),
            currency: crate::Currency::Krw,
            market_capitalization: Some(crate::NumericInput::try_new("200").expect("금액")),
            trading_amount: Some(crate::NumericInput::try_new("300").expect("금액")),
        };
        let fact = SectorComposition::from_stocks_response(
            SectorId::try_new("100").expect("섹터"),
            KnownMarket::Us,
            vec![input()],
            stamp,
        )
        .expect("미국도 원화");
        let member = fact.stocks().first().expect("구성");
        assert_eq!(member.symbol().as_str(), "ABCDEF");
        assert_eq!(
            member.market_capitalization(),
            crate::SectorAmount::Available(
                crate::Money::parse(
                    &crate::NumericInput::try_new("200").expect("금액"),
                    crate::Currency::Krw
                )
                .expect("원화")
            )
        );
        assert_eq!(
            member.trading_amount(),
            crate::SectorAmount::Available(
                crate::Money::parse(
                    &crate::NumericInput::try_new("300").expect("금액"),
                    crate::Currency::Krw
                )
                .expect("원화")
            )
        );
    }
    #[test]
    fn composition_rejects_bad_money_and_preserves_missing_values() {
        let stamp = Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        let input = || crate::SectorStockRow {
            symbol: Symbol::try_new("ABCDEF").expect("종목"),
            currency: crate::Currency::Krw,
            market_capitalization: Some(crate::NumericInput::try_new("200").expect("금액")),
            trading_amount: Some(crate::NumericInput::try_new("300").expect("금액")),
        };
        for field in ["cap", "amount", "currency"] {
            let mut bad = input();
            if field == "cap" {
                bad.market_capitalization =
                    Some(crate::NumericInput::try_new("bad").expect("원문"));
            } else if field == "amount" {
                bad.trading_amount = Some(crate::NumericInput::try_new("-1").expect("원문"));
            } else {
                bad.currency = crate::Currency::Usd;
            }
            SectorComposition::from_stocks_response(
                SectorId::try_new("100").expect("섹터"),
                KnownMarket::Us,
                vec![bad],
                stamp,
            )
            .expect_err("잘못된 금액");
        }
        let mut missing = input();
        missing.market_capitalization = None;
        missing.trading_amount = None;
        let absent = SectorComposition::from_stocks_response(
            SectorId::try_new("100").expect("섹터"),
            KnownMarket::Us,
            vec![missing],
            stamp,
        )
        .expect("미제공");
        assert_eq!(
            absent
                .stocks()
                .first()
                .expect("구성")
                .market_capitalization(),
            crate::SectorAmount::Unavailable
        );
        assert_eq!(
            absent.stocks().first().expect("구성").trading_amount(),
            crate::SectorAmount::Unavailable
        );
    }
    #[test]
    fn same_symbol_in_another_market_is_the_only_membership_mismatch() {
        let stamp = Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        let mut raw = master("ABCDEF", "KOSPI").row().clone();
        raw.listing_market = crate::MasterCode::try_new("NASDAQ").expect("시장");
        raw.currency = crate::Currency::Usd;
        let foreign = crate::MasterInstrument::try_new(raw).expect("미국 마스터");
        let domestic = SectorComposition::from_stocks_response(
            SectorId::try_new("100").expect("섹터"),
            KnownMarket::Kr,
            rows(&[Symbol::try_new("ABCDEF").expect("종목")]),
            stamp,
        )
        .expect("같은 심볼");
        assert_eq!(
            domestic.symbols().first(),
            Some(foreign.quote_instrument().expect("식별").symbol())
        );
        domestic
            .identified_member(&foreign)
            .expect_err("시장만 불일치");
    }
}
